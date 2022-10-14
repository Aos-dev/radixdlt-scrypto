use crate::types::*;
use scrypto::core::NativeFunction;

#[derive(Debug, Clone, Eq, PartialEq, TypeId, Encode, Decode)]
pub enum ResolvedMethod {
    Scrypto {
        package_address: PackageAddress,
        blueprint_name: String,
        ident: String,
        export_name: String,
    },
    Native(NativeMethod),
}

#[derive(Debug, Clone, Eq, PartialEq, TypeId, Encode, Decode)]
pub struct ResolvedReceiverMethod {
    pub receiver: Receiver,
    pub method: ResolvedMethod,
}

#[derive(Debug, Clone, Eq, PartialEq, TypeId, Encode, Decode)]
pub enum ResolvedFunction {
    Scrypto {
        package_address: PackageAddress,
        blueprint_name: String,
        ident: String,
        export_name: String,
    },
    Native(NativeFunction),
}

#[derive(Debug, Clone, Eq, PartialEq, TypeId, Encode, Decode)]
pub enum REActor {
    Function(ResolvedFunction),
    Method(ResolvedReceiverMethod),
}

impl REActor {
    pub fn is_scrypto_or_transaction(&self) -> bool {
        matches!(
            self,
            REActor::Method(ResolvedReceiverMethod {
                method: ResolvedMethod::Scrypto { .. },
                ..
            }) | REActor::Function(ResolvedFunction::Scrypto { .. })
                | REActor::Function(ResolvedFunction::Native(
                    NativeFunction::TransactionProcessor(TransactionProcessorFunction::Run)
                ))
        )
    }

    pub fn is_substate_readable(&self, node_id: RENodeId, offset: SubstateOffset) -> bool {
        match self {
            REActor::Function(ResolvedFunction::Native(..))
            | REActor::Method(ResolvedReceiverMethod {
                method: ResolvedMethod::Native(..),
                ..
            }) => true,
            REActor::Function(ResolvedFunction::Scrypto { .. }) => match (node_id, offset) {
                (
                    RENodeId::KeyValueStore(_),
                    SubstateOffset::KeyValueStore(KeyValueStoreOffset::Entry(..)),
                ) => true,
                (RENodeId::Global(_), SubstateOffset::Global(GlobalOffset::Global)) => true,
                (RENodeId::Component(_), SubstateOffset::Component(ComponentOffset::Info)) => true,
                _ => false,
            },
            REActor::Method(ResolvedReceiverMethod {
                receiver: Receiver::Ref(RENodeId::Component(component_address)),
                method: ResolvedMethod::Scrypto { .. },
            }) => match (node_id, offset) {
                (
                    RENodeId::KeyValueStore(_),
                    SubstateOffset::KeyValueStore(KeyValueStoreOffset::Entry(..)),
                ) => true,
                (RENodeId::Component(_), SubstateOffset::Component(ComponentOffset::Info)) => true,
                (RENodeId::Component(addr), SubstateOffset::Component(ComponentOffset::State)) => {
                    addr.eq(component_address)
                }
                (RENodeId::Global(_), SubstateOffset::Global(GlobalOffset::Global)) => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_substate_writeable(&self, node_id: RENodeId, offset: SubstateOffset) -> bool {
        match self {
            REActor::Function(ResolvedFunction::Native(..))
            | REActor::Method(ResolvedReceiverMethod {
                method: ResolvedMethod::Native(..),
                ..
            }) => true,
            REActor::Function(ResolvedFunction::Scrypto { .. }) => match (node_id, offset) {
                (
                    RENodeId::KeyValueStore(_),
                    SubstateOffset::KeyValueStore(KeyValueStoreOffset::Entry(..)),
                ) => true,
                _ => false,
            },
            REActor::Method(ResolvedReceiverMethod {
                receiver: Receiver::Ref(RENodeId::Component(component_address)),
                method: ResolvedMethod::Scrypto { .. },
            }) => match (node_id, offset) {
                (
                    RENodeId::KeyValueStore(_),
                    SubstateOffset::KeyValueStore(KeyValueStoreOffset::Entry(..)),
                ) => true,
                (RENodeId::Component(addr), SubstateOffset::Component(ComponentOffset::State)) => {
                    addr.eq(component_address)
                }
                _ => false,
            },
            _ => false,
        }
    }
}
