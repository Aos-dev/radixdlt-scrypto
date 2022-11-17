pub use radix_engine_lib::address::{AddressError, Bech32Decoder, Bech32Encoder};
pub use radix_engine_lib::crypto::Blob;
pub use radix_engine_lib::crypto::{
    EcdsaSecp256k1PublicKey, EcdsaSecp256k1Signature, EddsaEd25519PublicKey, EddsaEd25519Signature,
    Hash, PublicKey, Signature,
};
use radix_engine_lib::data::IndexedScryptoValue;
pub use radix_engine_lib::dec;
pub use radix_engine_lib::engine::actor::ScryptoActor;
use radix_engine_lib::engine::types::{
    NativeMethod, RENodeId, ScryptoFunctionIdent, ScryptoMethodIdent,
};
pub use radix_engine_lib::engine::{scrypto_env::RadixEngineInput, types::*};
pub use radix_engine_lib::math::{Decimal, RoundingMode, I256};
pub use radix_engine_lib::model::*;

pub use sbor::rust::borrow::ToOwned;
pub use sbor::rust::boxed::Box;
pub use sbor::rust::cell::{Ref, RefCell, RefMut};
pub use sbor::rust::collections::*;
pub use sbor::rust::fmt;
pub use sbor::rust::format;
pub use sbor::rust::marker::PhantomData;
pub use sbor::rust::ops::AddAssign;
pub use sbor::rust::ptr;
pub use sbor::rust::rc::Rc;
pub use sbor::rust::str::FromStr;
pub use sbor::rust::string::String;
pub use sbor::rust::string::ToString;
pub use sbor::rust::vec;
pub use sbor::rust::vec::Vec;
pub use sbor::{Decode, DecodeError, Encode, SborPath, SborPathBuf, SborTypeId, SborValue, TypeId};
pub use scrypto::abi::{BlueprintAbi, Fields, Fn, Type, Variant};
pub use scrypto::access_and_or;
pub use scrypto::access_rule_node;

pub use scrypto::constants::*;
pub use scrypto::core::Expression;
pub use scrypto::rule;
pub use scrypto::scrypto;
use std::fmt::Debug;

// methods and macros
use crate::engine::Invocation;
pub use sbor::decode_any;
pub use scrypto::buffer::{scrypto_decode, scrypto_encode};

pub use scrypto::args;

/// Scrypto function/method invocation.
#[derive(Debug)]
pub enum ScryptoInvocation {
    Function(ScryptoFunctionIdent, IndexedScryptoValue),
    Method(ScryptoMethodIdent, IndexedScryptoValue),
}

impl Invocation for ScryptoInvocation {
    type Output = IndexedScryptoValue;
}

impl ScryptoInvocation {
    pub fn args(&self) -> &IndexedScryptoValue {
        match self {
            ScryptoInvocation::Function(_, args) => &args,
            ScryptoInvocation::Method(_, args) => &args,
        }
    }
}

#[derive(Debug)]
pub struct NativeMethodInvocation(pub NativeMethod, pub RENodeId, pub IndexedScryptoValue);

impl Invocation for NativeMethodInvocation {
    type Output = IndexedScryptoValue;
}

impl NativeMethodInvocation {
    pub fn args(&self) -> &IndexedScryptoValue {
        &self.2
    }
}
