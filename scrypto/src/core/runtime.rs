use crate::buffer::{scrypto_decode, scrypto_encode};
use crate::call_data_bytes_args;
use crate::component::*;
use crate::core::*;
use crate::crypto::*;
use crate::engine::{api::*, call_engine};
use crate::rust::borrow::ToOwned;
use crate::rust::vec::Vec;
use sbor::*;

#[derive(Debug, TypeId, Encode, Decode)]
pub enum SystemFunction {
    GetEpoch(),
    GetTransactionHash(),
}

/// The transaction runtime.
#[derive(Debug)]
pub struct Runtime {}

impl Runtime {
    /// Returns the running entity, a component if within a call-method context or a
    /// blueprint if within a call-function context.
    pub fn actor() -> ScryptoActorInfo {
        let input = GetActorInput {};
        let output: GetActorOutput = call_engine(GET_ACTOR, input);
        output.actor
    }

    /// Returns the package ID.
    pub fn package_address() -> PackageAddress {
        let input = GetActorInput {};
        let output: GetActorOutput = call_engine(GET_ACTOR, input);
        output.actor.to_package_address()
    }

    /// Generates a UUID.
    pub fn generate_uuid() -> u128 {
        let input = GenerateUuidInput {};
        let output: GenerateUuidOutput = call_engine(GENERATE_UUID, input);

        output.uuid
    }

    /// Invokes a function on a blueprint.
    pub fn call_function<S: AsRef<str>>(
        package_address: PackageAddress,
        blueprint_name: S,
        function: S,
        args: Vec<Vec<u8>>,
    ) -> Vec<u8> {
        let call_data = call_data_bytes_args!(function.as_ref().to_owned(), args);
        let input = InvokeSNodeInput {
            snode_ref: SNodeRef::Scrypto(ScryptoActor::Blueprint(
                package_address,
                blueprint_name.as_ref().to_owned(),
            )),
            call_data,
        };
        let output: InvokeSNodeOutput = call_engine(INVOKE_SNODE, input);

        output.rtn
    }

    /// Invokes a method on a component.
    pub fn call_method<S: AsRef<str>>(
        component_address: ComponentAddress,
        method: S,
        args: Vec<Vec<u8>>,
    ) -> Vec<u8> {
        let mut fields = Vec::new();
        for arg in args {
            fields.push(::sbor::decode_any(&arg).unwrap());
        }
        let variant = ::sbor::Value::Enum {
            name: method.as_ref().to_owned(),
            fields,
        };

        let input = InvokeSNodeInput {
            snode_ref: SNodeRef::Scrypto(ScryptoActor::Component(component_address)),
            call_data: ::sbor::encode_any(&variant),
        };
        let output: InvokeSNodeOutput = call_engine(INVOKE_SNODE, input);

        output.rtn
    }

    /// Returns the transaction hash.
    pub fn transaction_hash() -> Hash {
        let input = InvokeSNodeInput {
            snode_ref: SNodeRef::SystemStatic,
            call_data: scrypto_encode(&SystemFunction::GetTransactionHash()),
        };
        let output: InvokeSNodeOutput = call_engine(INVOKE_SNODE, input);
        scrypto_decode(&output.rtn).unwrap()
    }

    /// Returns the current epoch number.
    pub fn current_epoch() -> u64 {
        let input = InvokeSNodeInput {
            snode_ref: SNodeRef::SystemStatic,
            call_data: scrypto_encode(&SystemFunction::GetEpoch()),
        };
        let output: InvokeSNodeOutput = call_engine(INVOKE_SNODE, input);
        scrypto_decode(&output.rtn).unwrap()
    }
}
