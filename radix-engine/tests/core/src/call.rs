use scrypto::prelude::*;

blueprint! {
    struct MoveTest {
        vaults: Vec<Vault>,
    }

    impl MoveTest {
        fn create_test_token(amount: u32) -> Bucket {
            ResourceBuilder::new_fungible(DIVISIBILITY_MAXIMUM)
                .metadata("name", "TestToken")
                .initial_supply_fungible(amount)
        }

        pub fn receive_bucket(&mut self, t: Bucket) {
            self.vaults.push(Vault::with_bucket(t));
        }

        pub fn receive_proof(&self, t: Proof) {
            t.drop();
        }

        pub fn move_bucket() {
            let bucket = Self::create_test_token(1000);
            let component_id = MoveTest { vaults: Vec::new() }.instantiate();
            Process::call_method(component_id, "receive_bucket", args!(bucket));
        }

        pub fn move_proof() -> Bucket {
            let bucket = Self::create_test_token(1000);
            let component_id = MoveTest { vaults: Vec::new() }.instantiate();
            Process::call_method(component_id, "receive_proof", args!(bucket.present()));

            bucket
        }
    }
}