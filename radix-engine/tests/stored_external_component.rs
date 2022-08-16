use radix_engine::ledger::TypedInMemorySubstateStore;
use scrypto::core::Network;
use scrypto::prelude::*;
use scrypto::to_struct;
use scrypto_unit::*;
use transaction::builder::ManifestBuilder;

#[test]
fn stored_component_addresses_are_invokable() {
    // Arrange
    let mut store = TypedInMemorySubstateStore::with_bootstrap();
    let mut test_runner = TestRunner::new(true, &mut store);
    let (public_key, _, _) = test_runner.new_account();
    let package = test_runner.extract_and_publish_package("stored_external_component");
    let manifest1 = ManifestBuilder::new(Network::LocalSimulator)
        .lock_fee(10.into(), SYSTEM_COMPONENT)
        .call_function(package, "ExternalComponent", "create", to_struct!())
        .build();
    let receipt1 = test_runner.execute_manifest(manifest1, vec![]);
    receipt1.expect_success();
    let component0 = receipt1.new_component_addresses[0];
    let component1 = receipt1.new_component_addresses[1];

    // Act
    let manifest2 = ManifestBuilder::new(Network::LocalSimulator)
        .lock_fee(10.into(), SYSTEM_COMPONENT)
        .call_method(component0, "func", to_struct!())
        .build();
    let receipt2 = test_runner.execute_manifest(manifest2, vec![public_key]);

    // Assert
    receipt2.expect_success();

    // Act
    let manifest2 = ManifestBuilder::new(Network::LocalSimulator)
        .lock_fee(10.into(), SYSTEM_COMPONENT)
        .call_method(component1, "func", to_struct!())
        .build();
    let receipt2 = test_runner.execute_manifest(manifest2, vec![public_key]);

    // Assert
    receipt2.expect_success();
}