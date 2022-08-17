use sbor::rust::collections::HashMap;
use sbor::rust::string::ToString;
use sbor::rust::vec::Vec;
use sbor::{Decode, Encode, TypeId};
use scrypto::args;
use scrypto::buffer::scrypto_decode;
use scrypto::component::Package;
use scrypto::core::{
    AuthZoneFnIdentifier, BucketFnIdentifier, FnIdentifier, NativeFnIdentifier,
    PackageFnIdentifier, ProofFnIdentifier, Receiver, WorktopFnIdentifier,
};
use scrypto::engine::types::*;
use scrypto::prelude::{
    AuthZoneClearInput, AuthZoneCreateProofByAmountInput, AuthZoneCreateProofByIdsInput,
    AuthZoneCreateProofInput, AuthZonePushInput, BucketCreateProofInput, PackagePublishInput,
    ProofCloneInput, TransactionProcessorFnIdentifier,
};
use scrypto::resource::{AuthZonePopInput, ConsumingProofDropInput};
use scrypto::values::*;
use transaction::model::*;
use transaction::validation::*;

use crate::engine::{HeapRENode, RuntimeError, RuntimeError::ProofNotFound, SystemApi};
use crate::fee::FeeReserve;
use crate::model::worktop::{
    WorktopAssertContainsAmountInput, WorktopAssertContainsInput,
    WorktopAssertContainsNonFungiblesInput, WorktopDrainInput, WorktopPutInput,
    WorktopTakeAllInput, WorktopTakeAmountInput, WorktopTakeNonFungiblesInput,
};
use crate::wasm::*;

use super::Worktop;

#[derive(Debug, TypeId, Encode, Decode)]
pub struct TransactionProcessorRunInput {
    pub instructions: Vec<ExecutableInstruction>,
}

#[derive(Debug)]
pub enum TransactionProcessorError {
    InvalidRequestData(DecodeError),
    RuntimeError(RuntimeError),
    InvalidMethod,
}

pub struct TransactionProcessor {}

impl TransactionProcessor {
    fn replace_ids(
        proof_id_mapping: &mut HashMap<ProofId, ProofId>,
        bucket_id_mapping: &mut HashMap<BucketId, BucketId>,
        mut value: ScryptoValue,
    ) -> Result<ScryptoValue, RuntimeError> {
        value
            .replace_ids(proof_id_mapping, bucket_id_mapping)
            .map_err(|e| match e {
                ScryptoValueReplaceError::BucketIdNotFound(bucket_id) => {
                    RuntimeError::BucketNotFound(bucket_id)
                }
                ScryptoValueReplaceError::ProofIdNotFound(proof_id) => {
                    RuntimeError::ProofNotFound(proof_id)
                }
            })?;
        Ok(value)
    }

    pub fn static_main<
        's,
        Y: SystemApi<'s, W, I, C>,
        W: WasmEngine<I>,
        I: WasmInstance,
        C: FeeReserve,
    >(
        transaction_processor_fn: TransactionProcessorFnIdentifier,
        call_data: ScryptoValue,
        system_api: &mut Y,
    ) -> Result<ScryptoValue, TransactionProcessorError> {
        match transaction_processor_fn {
            TransactionProcessorFnIdentifier::Run => {
                let input: TransactionProcessorRunInput = scrypto_decode(&call_data.raw)
                    .map_err(|e| TransactionProcessorError::InvalidRequestData(e))?;
                let mut proof_id_mapping = HashMap::new();
                let mut bucket_id_mapping = HashMap::new();
                let mut outputs = Vec::new();
                let mut id_allocator = IdAllocator::new(IdSpace::Transaction);

                let _worktop_id = system_api
                    .node_create(HeapRENode::Worktop(Worktop::new()))
                    .expect("Should never fail.");

                for inst in &input.instructions.clone() {
                    let result = match inst {
                        ExecutableInstruction::TakeFromWorktop { resource_address } => id_allocator
                            .new_bucket_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                system_api
                                    .invoke_method(
                                        Receiver::Ref(RENodeId::Worktop),
                                        FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                            WorktopFnIdentifier::TakeAll,
                                        )),
                                        ScryptoValue::from_typed(&WorktopTakeAllInput {
                                            resource_address: *resource_address,
                                        }),
                                    )
                                    .map(|rtn| {
                                        let bucket_id = *rtn.bucket_ids.iter().next().unwrap().0;
                                        bucket_id_mapping.insert(new_id, bucket_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Bucket(new_id))
                                    })
                            }),
                        ExecutableInstruction::TakeFromWorktopByAmount {
                            amount,
                            resource_address,
                        } => id_allocator
                            .new_bucket_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                system_api
                                    .invoke_method(
                                        Receiver::Ref(RENodeId::Worktop),
                                        FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                            WorktopFnIdentifier::TakeAmount,
                                        )),
                                        ScryptoValue::from_typed(&WorktopTakeAmountInput {
                                            amount: *amount,
                                            resource_address: *resource_address,
                                        }),
                                    )
                                    .map(|rtn| {
                                        let bucket_id = *rtn.bucket_ids.iter().next().unwrap().0;
                                        bucket_id_mapping.insert(new_id, bucket_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Bucket(new_id))
                                    })
                            }),
                        ExecutableInstruction::TakeFromWorktopByIds {
                            ids,
                            resource_address,
                        } => id_allocator
                            .new_bucket_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                system_api
                                    .invoke_method(
                                        Receiver::Ref(RENodeId::Worktop),
                                        FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                            WorktopFnIdentifier::TakeNonFungibles,
                                        )),
                                        ScryptoValue::from_typed(&WorktopTakeNonFungiblesInput {
                                            ids: ids.clone(),
                                            resource_address: *resource_address,
                                        }),
                                    )
                                    .map(|rtn| {
                                        let bucket_id = *rtn.bucket_ids.iter().next().unwrap().0;
                                        bucket_id_mapping.insert(new_id, bucket_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Bucket(new_id))
                                    })
                            }),
                        ExecutableInstruction::ReturnToWorktop { bucket_id } => bucket_id_mapping
                            .remove(bucket_id)
                            .map(|real_id| {
                                system_api.invoke_method(
                                    Receiver::Ref(RENodeId::Worktop),
                                    FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                        WorktopFnIdentifier::Put,
                                    )),
                                    ScryptoValue::from_typed(&WorktopPutInput {
                                        bucket: scrypto::resource::Bucket(real_id),
                                    }),
                                )
                            })
                            .unwrap_or(Err(RuntimeError::BucketNotFound(*bucket_id))),
                        ExecutableInstruction::AssertWorktopContains { resource_address } => {
                            system_api.invoke_method(
                                Receiver::Ref(RENodeId::Worktop),
                                FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                    WorktopFnIdentifier::AssertContains,
                                )),
                                ScryptoValue::from_typed(&WorktopAssertContainsInput {
                                    resource_address: *resource_address,
                                }),
                            )
                        }
                        ExecutableInstruction::AssertWorktopContainsByAmount {
                            amount,
                            resource_address,
                        } => system_api.invoke_method(
                            Receiver::Ref(RENodeId::Worktop),
                            FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                WorktopFnIdentifier::AssertContainsAmount,
                            )),
                            ScryptoValue::from_typed(&WorktopAssertContainsAmountInput {
                                amount: *amount,
                                resource_address: *resource_address,
                            }),
                        ),
                        ExecutableInstruction::AssertWorktopContainsByIds {
                            ids,
                            resource_address,
                        } => system_api.invoke_method(
                            Receiver::Ref(RENodeId::Worktop),
                            FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                WorktopFnIdentifier::AssertContainsNonFungibles,
                            )),
                            ScryptoValue::from_typed(&WorktopAssertContainsNonFungiblesInput {
                                ids: ids.clone(),
                                resource_address: *resource_address,
                            }),
                        ),

                        ExecutableInstruction::PopFromAuthZone {} => id_allocator
                            .new_proof_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                system_api
                                    .invoke_method(
                                        Receiver::CurrentAuthZone,
                                        FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                            AuthZoneFnIdentifier::Pop,
                                        )),
                                        ScryptoValue::from_typed(&AuthZonePopInput {}),
                                    )
                                    .map(|rtn| {
                                        let proof_id = *rtn.proof_ids.iter().next().unwrap().0;
                                        proof_id_mapping.insert(new_id, proof_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Proof(new_id))
                                    })
                            }),
                        ExecutableInstruction::ClearAuthZone => {
                            proof_id_mapping.clear();
                            system_api.invoke_method(
                                Receiver::CurrentAuthZone,
                                FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                    AuthZoneFnIdentifier::Clear,
                                )),
                                ScryptoValue::from_typed(&AuthZoneClearInput {}),
                            )
                        }
                        ExecutableInstruction::PushToAuthZone { proof_id } => proof_id_mapping
                            .remove(proof_id)
                            .ok_or(RuntimeError::ProofNotFound(*proof_id))
                            .and_then(|real_id| {
                                system_api.invoke_method(
                                    Receiver::CurrentAuthZone,
                                    FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                        AuthZoneFnIdentifier::Push,
                                    )),
                                    ScryptoValue::from_typed(&AuthZonePushInput {
                                        proof: scrypto::resource::Proof(real_id),
                                    }),
                                )
                            }),
                        ExecutableInstruction::CreateProofFromAuthZone { resource_address } => {
                            id_allocator
                                .new_proof_id()
                                .map_err(RuntimeError::IdAllocationError)
                                .and_then(|new_id| {
                                    system_api
                                        .invoke_method(
                                            Receiver::CurrentAuthZone,
                                            FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                                AuthZoneFnIdentifier::CreateProof,
                                            )),
                                            ScryptoValue::from_typed(&AuthZoneCreateProofInput {
                                                resource_address: *resource_address,
                                            }),
                                        )
                                        .map(|rtn| {
                                            let proof_id = *rtn.proof_ids.iter().next().unwrap().0;
                                            proof_id_mapping.insert(new_id, proof_id);
                                            ScryptoValue::from_typed(&scrypto::resource::Proof(
                                                new_id,
                                            ))
                                        })
                                })
                        }
                        ExecutableInstruction::CreateProofFromAuthZoneByAmount {
                            amount,
                            resource_address,
                        } => id_allocator
                            .new_proof_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                system_api
                                    .invoke_method(
                                        Receiver::CurrentAuthZone,
                                        FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                            AuthZoneFnIdentifier::CreateProofByAmount,
                                        )),
                                        ScryptoValue::from_typed(
                                            &AuthZoneCreateProofByAmountInput {
                                                amount: *amount,
                                                resource_address: *resource_address,
                                            },
                                        ),
                                    )
                                    .map(|rtn| {
                                        let proof_id = *rtn.proof_ids.iter().next().unwrap().0;
                                        proof_id_mapping.insert(new_id, proof_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Proof(new_id))
                                    })
                            }),
                        ExecutableInstruction::CreateProofFromAuthZoneByIds {
                            ids,
                            resource_address,
                        } => id_allocator
                            .new_proof_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                system_api
                                    .invoke_method(
                                        Receiver::CurrentAuthZone,
                                        FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                            AuthZoneFnIdentifier::CreateProofByIds,
                                        )),
                                        ScryptoValue::from_typed(&AuthZoneCreateProofByIdsInput {
                                            ids: ids.clone(),
                                            resource_address: *resource_address,
                                        }),
                                    )
                                    .map(|rtn| {
                                        let proof_id = *rtn.proof_ids.iter().next().unwrap().0;
                                        proof_id_mapping.insert(new_id, proof_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Proof(new_id))
                                    })
                            }),
                        ExecutableInstruction::CreateProofFromBucket { bucket_id } => id_allocator
                            .new_proof_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                bucket_id_mapping
                                    .get(bucket_id)
                                    .cloned()
                                    .map(|real_bucket_id| (new_id, real_bucket_id))
                                    .ok_or(RuntimeError::BucketNotFound(new_id))
                            })
                            .and_then(|(new_id, real_bucket_id)| {
                                system_api
                                    .invoke_method(
                                        Receiver::Ref(RENodeId::Bucket(real_bucket_id)),
                                        FnIdentifier::Native(NativeFnIdentifier::Bucket(
                                            BucketFnIdentifier::CreateProof,
                                        )),
                                        ScryptoValue::from_typed(&BucketCreateProofInput {}),
                                    )
                                    .map(|rtn| {
                                        let proof_id = *rtn.proof_ids.iter().next().unwrap().0;
                                        proof_id_mapping.insert(new_id, proof_id);
                                        ScryptoValue::from_typed(&scrypto::resource::Proof(new_id))
                                    })
                            }),
                        ExecutableInstruction::CloneProof { proof_id } => id_allocator
                            .new_proof_id()
                            .map_err(RuntimeError::IdAllocationError)
                            .and_then(|new_id| {
                                proof_id_mapping
                                    .get(proof_id)
                                    .cloned()
                                    .map(|real_id| {
                                        system_api
                                            .invoke_method(
                                                Receiver::Ref(RENodeId::Proof(real_id)),
                                                FnIdentifier::Native(NativeFnIdentifier::Proof(
                                                    ProofFnIdentifier::Clone,
                                                )),
                                                ScryptoValue::from_typed(&ProofCloneInput {}),
                                            )
                                            .map(|v| {
                                                let cloned_proof_id =
                                                    v.proof_ids.iter().next().unwrap().0;
                                                proof_id_mapping.insert(new_id, *cloned_proof_id);
                                                ScryptoValue::from_typed(&scrypto::resource::Proof(
                                                    new_id,
                                                ))
                                            })
                                    })
                                    .unwrap_or(Err(RuntimeError::ProofNotFound(*proof_id)))
                            }),
                        ExecutableInstruction::DropProof { proof_id } => proof_id_mapping
                            .remove(proof_id)
                            .map(|real_id| {
                                system_api.invoke_method(
                                    Receiver::Consumed(RENodeId::Proof(real_id)),
                                    FnIdentifier::Native(NativeFnIdentifier::Proof(
                                        ProofFnIdentifier::Drop,
                                    )),
                                    ScryptoValue::from_typed(&ConsumingProofDropInput {}),
                                )
                            })
                            .unwrap_or(Err(ProofNotFound(*proof_id))),
                        ExecutableInstruction::DropAllProofs => {
                            for (_, real_id) in proof_id_mapping.drain() {
                                system_api
                                    .invoke_method(
                                        Receiver::Consumed(RENodeId::Proof(real_id)),
                                        FnIdentifier::Native(NativeFnIdentifier::Proof(
                                            ProofFnIdentifier::Drop,
                                        )),
                                        ScryptoValue::from_typed(&ConsumingProofDropInput {}),
                                    )
                                    .unwrap();
                            }
                            system_api.invoke_method(
                                Receiver::CurrentAuthZone,
                                FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                    AuthZoneFnIdentifier::Clear,
                                )),
                                ScryptoValue::from_typed(&AuthZoneClearInput {}),
                            )
                        }
                        ExecutableInstruction::CallFunction {
                            package_address,
                            blueprint_name,
                            method_name,
                            args,
                        } => {
                            Self::replace_ids(
                                &mut proof_id_mapping,
                                &mut bucket_id_mapping,
                                ScryptoValue::from_slice(args).expect("Should be valid arg"),
                            )
                            .and_then(|call_data| {
                                system_api.invoke_function(
                                    FnIdentifier::Scrypto {
                                        package_address: *package_address,
                                        blueprint_name: blueprint_name.to_string(),
                                        ident: method_name.to_string(),
                                    },
                                    call_data,
                                )
                            })
                            .and_then(|result| {
                                // Auto move into auth_zone
                                for (proof_id, _) in &result.proof_ids {
                                    system_api
                                        .invoke_method(
                                            Receiver::CurrentAuthZone,
                                            FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                                AuthZoneFnIdentifier::Push,
                                            )),
                                            ScryptoValue::from_typed(&AuthZonePushInput {
                                                proof: scrypto::resource::Proof(*proof_id),
                                            }),
                                        )
                                        .unwrap(); // TODO: Remove unwrap
                                }
                                // Auto move into worktop
                                for (bucket_id, _) in &result.bucket_ids {
                                    system_api
                                        .invoke_method(
                                            Receiver::Ref(RENodeId::Worktop),
                                            FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                                WorktopFnIdentifier::Put,
                                            )),
                                            ScryptoValue::from_typed(&WorktopPutInput {
                                                bucket: scrypto::resource::Bucket(*bucket_id),
                                            }),
                                        )
                                        .unwrap(); // TODO: Remove unwrap
                                }
                                Ok(result)
                            })
                        }
                        ExecutableInstruction::CallMethod {
                            component_address,
                            method_name,
                            args,
                        } => {
                            Self::replace_ids(
                                &mut proof_id_mapping,
                                &mut bucket_id_mapping,
                                ScryptoValue::from_slice(args).expect("Should be valid arg"),
                            )
                            .and_then(|call_data| {
                                // TODO: Move this into preprocessor step
                                system_api
                                    .substate_read(SubstateId::ComponentInfo(*component_address))
                                    .and_then(|s| {
                                        let (package_address, blueprint_name): (
                                            PackageAddress,
                                            String,
                                        ) = scrypto_decode(&s.raw).expect("Should not fail.");
                                        system_api.invoke_method(
                                            Receiver::Ref(RENodeId::Component(*component_address)),
                                            FnIdentifier::Scrypto {
                                                ident: method_name.to_string(),
                                                package_address,
                                                blueprint_name,
                                            },
                                            call_data,
                                        )
                                    })
                            })
                            .and_then(|result| {
                                // Auto move into auth_zone
                                for (proof_id, _) in &result.proof_ids {
                                    system_api
                                        .invoke_method(
                                            Receiver::CurrentAuthZone,
                                            FnIdentifier::Native(NativeFnIdentifier::AuthZone(
                                                AuthZoneFnIdentifier::Push,
                                            )),
                                            ScryptoValue::from_typed(&AuthZonePushInput {
                                                proof: scrypto::resource::Proof(*proof_id),
                                            }),
                                        )
                                        .unwrap();
                                }
                                // Auto move into worktop
                                for (bucket_id, _) in &result.bucket_ids {
                                    system_api
                                        .invoke_method(
                                            Receiver::Ref(RENodeId::Worktop),
                                            FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                                WorktopFnIdentifier::Put,
                                            )),
                                            ScryptoValue::from_typed(&WorktopPutInput {
                                                bucket: scrypto::resource::Bucket(*bucket_id),
                                            }),
                                        )
                                        .unwrap(); // TODO: Remove unwrap
                                }
                                Ok(result)
                            })
                        }
                        ExecutableInstruction::CallMethodWithAllResources {
                            component_address,
                            method,
                        } => system_api
                            .invoke_method(
                                Receiver::Ref(RENodeId::Worktop),
                                FnIdentifier::Native(NativeFnIdentifier::Worktop(
                                    WorktopFnIdentifier::Drain,
                                )),
                                ScryptoValue::from_typed(&WorktopDrainInput {}),
                            )
                            .and_then(|result| {
                                let mut buckets = Vec::new();
                                for (bucket_id, _) in result.bucket_ids {
                                    buckets.push(scrypto::resource::Bucket(bucket_id));
                                }
                                for (_, real_id) in bucket_id_mapping.drain() {
                                    buckets.push(scrypto::resource::Bucket(real_id));
                                }
                                let encoded = args!(buckets);
                                // TODO: Move this into preprocessor step
                                system_api
                                    .substate_read(SubstateId::ComponentInfo(*component_address))
                                    .and_then(|s| {
                                        let (package_address, blueprint_name): (
                                            PackageAddress,
                                            String,
                                        ) = scrypto_decode(&s.raw).expect("Should not fail.");
                                        system_api.invoke_method(
                                            Receiver::Ref(RENodeId::Component(*component_address)),
                                            FnIdentifier::Scrypto {
                                                package_address,
                                                blueprint_name,
                                                ident: method.to_string(),
                                            },
                                            ScryptoValue::from_slice(&encoded).unwrap(),
                                        )
                                    })
                            }),
                        ExecutableInstruction::PublishPackage { package } => {
                            scrypto_decode::<Package>(package)
                                .map_err(|e| RuntimeError::InvalidPackage(e))
                                .and_then(|package| {
                                    system_api.invoke_function(
                                        FnIdentifier::Native(NativeFnIdentifier::Package(
                                            PackageFnIdentifier::Publish,
                                        )),
                                        ScryptoValue::from_typed(&PackagePublishInput { package }),
                                    )
                                })
                        }
                    }
                    .map_err(TransactionProcessorError::RuntimeError)?;
                    outputs.push(result);
                }

                Ok(ScryptoValue::from_typed(
                    &outputs
                        .into_iter()
                        .map(|sv| sv.raw)
                        .collect::<Vec<Vec<u8>>>(),
                ))
            }
        }
    }
}
