use sbor::describe::*;
use sbor::*;
use scrypto::abi;
use scrypto::buffer::*;
use scrypto::rust::borrow::ToOwned;
use scrypto::rust::collections::*;
use scrypto::rust::fmt;
use scrypto::rust::str::FromStr;
use scrypto::rust::string::String;
use scrypto::rust::string::ToString;
use scrypto::rust::vec;
use scrypto::rust::vec::Vec;
use scrypto::types::*;

use crate::engine::*;
use crate::transaction::*;

/// A utility for building transactions.
pub struct TransactionBuilder {
    /// The address allocator for calculating reserved bucket id.
    allocator: IdAllocator,
    /// Bucket or BucketRef reservations
    reservations: Vec<Instruction>,
    /// Instructions generated.
    instructions: Vec<Instruction>,
    /// Collected Errors
    errors: Vec<BuildTransactionError>,
}

impl TransactionBuilder {
    /// Starts a new transaction builder.
    pub fn new() -> Self {
        Self {
            allocator: IdAllocator::new(),
            reservations: Vec::new(),
            instructions: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Creates an empty bucket
    pub fn reserve_bucket(&mut self, resource_def: Address) -> BID {
        let bid = self.allocator.new_bid();
        self.reservations
            .push(Instruction::ReserveBucket { resource_def });
        bid
    }

    /// Creates a reference by borrowing a bucket.
    pub fn create_reference(&mut self, bucket: BID) -> RID {
        let rid = self.allocator.new_rid();
        self.reservations.push(Instruction::BorrowBucket { bucket });
        rid
    }

    /// Moves resource (from context) to a bucket.
    pub fn move_to_bucket(&mut self, amount: Amount, resource_def: Address, bucket: BID) {
        self.instruction(Instruction::MoveToBucket {
            amount,
            resource_def,
            bucket,
        });
    }

    /// Appends a raw instruction.
    pub fn instruction(&mut self, inst: Instruction) -> &mut Self {
        self.instructions.push(inst);
        self
    }

    /// Publishes a package.
    pub fn publish_package(&mut self, code: &[u8]) -> &mut Self {
        self.instruction(Instruction::CallFunction {
            blueprint: (SYSTEM_PACKAGE, "System".to_owned()),
            function: "publish_package".to_string(),
            args: vec![scrypto_encode(code)],
        })
    }

    /// Creates a resource with mutable supply.
    pub fn new_resource_mutable(&mut self, metadata: HashMap<String, String>) -> &mut Self {
        self.instruction(Instruction::CallFunction {
            blueprint: (SYSTEM_PACKAGE, "System".to_owned()),
            function: "new_resource_mutable".to_string(),
            args: vec![scrypto_encode(&metadata)],
        })
    }

    /// Creates a resource with fixed supply.
    pub fn new_resource_fixed(
        &mut self,
        metadata: HashMap<String, String>,
        supply: Amount,
    ) -> &mut Self {
        self.instruction(Instruction::CallFunction {
            blueprint: (SYSTEM_PACKAGE, "System".to_owned()),
            function: "new_resource_fixed".to_string(),
            args: vec![scrypto_encode(&metadata), scrypto_encode(&supply)],
        })
    }

    /// Mints resource.
    pub fn mint_resource(&mut self, amount: Amount, resource_def: Address) -> &mut Self {
        self.instruction(Instruction::CallFunction {
            blueprint: (SYSTEM_PACKAGE, "System".to_owned()),
            function: "mint_resource".to_string(),
            args: vec![scrypto_encode(&amount), scrypto_encode(&resource_def)],
        })
    }

    /// Creates an Account component.
    pub fn new_account(&mut self) -> &mut Self {
        self.instruction(Instruction::CallFunction {
            blueprint: (ACCOUNT_PACKAGE, "Account".to_owned()),
            function: "new".to_string(),
            args: vec![],
        })
    }

    /// Creates an Account component.
    pub fn new_account_with_resource(
        &mut self,
        amount: Amount,
        resource_def: Address,
    ) -> &mut Self {
        let bid = self.reserve_bucket(resource_def);
        self.move_to_bucket(amount, resource_def, bid);
        self.instruction(Instruction::CallFunction {
            blueprint: (ACCOUNT_PACKAGE, "Account".to_owned()),
            function: "with_bucket".to_string(),
            args: vec![scrypto_encode(&scrypto::resource::Bucket::from(bid))],
        })
    }

    /// Withdraws resource from an account.
    pub fn withdraw(
        &mut self,
        amount: Amount,
        resource_def: Address,
        account: Address,
    ) -> &mut Self {
        self.instruction(Instruction::CallMethod {
            component: account,
            method: "withdraw".to_string(),
            args: vec![scrypto_encode(&amount), scrypto_encode(&resource_def)],
        })
    }

    /// Deposits everything to an account.
    pub fn deposit_all(&mut self, account: Address) -> &mut Self {
        self.instruction(Instruction::DepositAll {
            component: account,
            method: "deposit_batch".to_string(),
        })
    }

    /// Calls a function.
    ///
    /// The default implementation will automatically prepare the arguments based on the
    /// function ABI, including resource buckets and references.
    ///
    /// If an account address is provided, resources will be withdrawn from the specified account;
    /// otherwise, they will be taken from transaction context (presumably obtained from
    /// previous instructions).
    pub fn call_function(
        &mut self,
        abi: &abi::Blueprint,
        function: &str,
        args: Vec<String>,
        account: Option<Address>,
    ) -> &mut Self {
        match Self::find_function_abi(abi, function.as_ref()) {
            Ok(f) => match self.prepare_args(&f.inputs, args, account) {
                Ok(o) => {
                    self.instructions.push(Instruction::CallFunction {
                        blueprint: (abi.package.parse().unwrap(), abi.name.clone()),
                        function: function.to_owned(),
                        args: o,
                    });
                }
                Err(e) => {
                    self.errors
                        .push(BuildTransactionError::FailedToBuildArgs(e));
                }
            },
            Err(e) => {
                self.errors.push(e);
            }
        }
        self
    }

    /// Calls a method.
    ///
    /// The default implementation will automatically prepare the arguments based on the
    /// method ABI, including resource buckets and references.
    ///
    /// If an account address is provided, resources will be withdrawn from the specified account;
    /// otherwise, they will be taken from transaction context (presumably obtained from
    /// previous instructions).
    pub fn call_method(
        &mut self,
        abi: &abi::Blueprint,
        component: Address,
        method: &str,
        args: Vec<String>,
        account: Option<Address>,
    ) -> &mut Self {
        match Self::find_method_abi(&abi, method.as_ref()) {
            Ok(m) => match self.prepare_args(&m.inputs, args, account) {
                Ok(o) => {
                    self.instructions.push(Instruction::CallMethod {
                        component,
                        method: method.to_owned(),
                        args: o,
                    });
                }
                Err(e) => {
                    self.errors
                        .push(BuildTransactionError::FailedToBuildArgs(e));
                }
            },
            Err(e) => {
                self.errors.push(e);
            }
        }
        self
    }

    /// Builds the transaction.
    pub fn build(&mut self) -> Result<Transaction, BuildTransactionError> {
        if !self.errors.is_empty() {
            return Err(self.errors[0].clone());
        }

        let mut v = Vec::new();
        v.extend(self.reservations.clone());
        v.extend(self.instructions.clone());
        v.push(Instruction::End);

        Ok(Transaction { instructions: v })
    }

    fn find_function_abi(
        abi: &abi::Blueprint,
        function: &str,
    ) -> Result<abi::Function, BuildTransactionError> {
        abi.functions
            .iter()
            .find(|f| f.name == function)
            .map(Clone::clone)
            .ok_or_else(|| BuildTransactionError::FunctionNotFound(function.to_owned()))
    }

    fn find_method_abi(
        abi: &abi::Blueprint,
        method: &str,
    ) -> Result<abi::Method, BuildTransactionError> {
        abi.methods
            .iter()
            .find(|m| m.name == method)
            .map(Clone::clone)
            .ok_or_else(|| BuildTransactionError::MethodNotFound(method.to_owned()))
    }

    fn prepare_args(
        &mut self,
        types: &[Type],
        args: Vec<String>,
        account: Option<Address>,
    ) -> Result<Vec<Vec<u8>>, BuildArgsError> {
        let mut encoded = Vec::new();

        for (i, t) in types.iter().enumerate() {
            let arg = args
                .get(i)
                .ok_or_else(|| BuildArgsError::MissingArgument(i, t.clone()))?;
            let res = match t {
                Type::Bool => self.prepare_basic_ty::<bool>(i, t, arg),
                Type::I8 => self.prepare_basic_ty::<i8>(i, t, arg),
                Type::I16 => self.prepare_basic_ty::<i16>(i, t, arg),
                Type::I32 => self.prepare_basic_ty::<i32>(i, t, arg),
                Type::I64 => self.prepare_basic_ty::<i64>(i, t, arg),
                Type::I128 => self.prepare_basic_ty::<i128>(i, t, arg),
                Type::U8 => self.prepare_basic_ty::<u8>(i, t, arg),
                Type::U16 => self.prepare_basic_ty::<u16>(i, t, arg),
                Type::U32 => self.prepare_basic_ty::<u32>(i, t, arg),
                Type::U64 => self.prepare_basic_ty::<u64>(i, t, arg),
                Type::U128 => self.prepare_basic_ty::<u128>(i, t, arg),
                Type::String => self.prepare_basic_ty::<String>(i, t, arg),
                Type::Custom { name } => self.prepare_custom_ty(i, t, arg, name, account),
                _ => Err(BuildArgsError::UnsupportedType(i, t.clone())),
            };
            encoded.push(res?);
        }

        Ok(encoded)
    }

    fn prepare_basic_ty<T>(
        &mut self,
        i: usize,
        ty: &Type,
        arg: &str,
    ) -> Result<Vec<u8>, BuildArgsError>
    where
        T: FromStr + Encode,
        T::Err: fmt::Debug,
    {
        let value = arg
            .parse::<T>()
            .map_err(|_| BuildArgsError::UnableToParse(i, ty.clone(), arg.to_owned()))?;
        Ok(scrypto_encode(&value))
    }

    fn prepare_custom_ty(
        &mut self,
        i: usize,
        ty: &Type,
        arg: &str,
        name: &str,
        account: Option<Address>,
    ) -> Result<Vec<u8>, BuildArgsError> {
        match name {
            SCRYPTO_NAME_AMOUNT => {
                let value = arg
                    .parse::<Amount>()
                    .map_err(|_| BuildArgsError::UnableToParse(i, ty.clone(), arg.to_owned()))?;
                Ok(scrypto_encode(&value))
            }
            SCRYPTO_NAME_ADDRESS => {
                let value = arg
                    .parse::<Address>()
                    .map_err(|_| BuildArgsError::UnableToParse(i, ty.clone(), arg.to_owned()))?;
                Ok(scrypto_encode(&value))
            }
            SCRYPTO_NAME_H256 => {
                let value = arg
                    .parse::<H256>()
                    .map_err(|_| BuildArgsError::UnableToParse(i, ty.clone(), arg.to_owned()))?;
                Ok(scrypto_encode(&value))
            }
            SCRYPTO_NAME_BID | SCRYPTO_NAME_BUCKET | SCRYPTO_NAME_RID | SCRYPTO_NAME_BUCKET_REF => {
                let mut split = arg.split(',');
                let amount = split.next().and_then(|v| v.trim().parse::<Amount>().ok());
                let resource_def = split.next().and_then(|v| v.trim().parse::<Address>().ok());
                match (amount, resource_def) {
                    (Some(a), Some(r)) => {
                        let bid = self.reserve_bucket(r);

                        if let Some(account) = account {
                            self.withdraw(a, r, account);
                        }
                        self.move_to_bucket(a, r, bid);

                        match name {
                            SCRYPTO_NAME_BID => Ok(scrypto_encode(&bid)),
                            SCRYPTO_NAME_BUCKET => {
                                Ok(scrypto_encode(&scrypto::resource::Bucket::from(bid)))
                            }
                            SCRYPTO_NAME_RID => {
                                let rid = self.create_reference(bid);
                                Ok(scrypto_encode(&rid))
                            }
                            SCRYPTO_NAME_BUCKET_REF => {
                                let rid = self.create_reference(bid);
                                Ok(scrypto_encode(&scrypto::resource::BucketRef::from(rid)))
                            }
                            _ => panic!("Unexpected"),
                        }
                    }
                    _ => Err(BuildArgsError::UnableToParse(i, ty.clone(), arg.to_owned())),
                }
            }
            _ => Err(BuildArgsError::UnsupportedType(i, ty.clone())),
        }
    }
}