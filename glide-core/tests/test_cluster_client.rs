/**
 * Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0
 */
mod utilities;

#[cfg(test)]
mod cluster_client_tests {
    use std::collections::HashMap;

    use super::*;
    use cluster::setup_cluster_with_replicas;
    use glide_core::connection_request::ReadFrom;
    use redis::cluster_routing::{
        MultipleNodeRoutingInfo, Route, RoutingInfo, SingleNodeRoutingInfo, SlotAddr,
    };
    use rstest::rstest;
    use utilities::cluster::{setup_test_basics_internal, SHORT_CLUSTER_TEST_TIMEOUT};
    use utilities::*;

    fn count_primary_or_replica(value: &str) -> (u16, u16) {
        if value.contains("role:master") {
            (1, 0)
        } else if value.contains("role:slave") {
            (0, 1)
        } else {
            (0, 0)
        }
    }

    fn count_primaries_and_replicas(info_replication: HashMap<String, String>) -> (u16, u16) {
        info_replication
            .into_iter()
            .fold((0, 0), |acc, (_, value)| {
                let count = count_primary_or_replica(&value);
                (acc.0 + count.0, acc.1 + count.1)
            })
    }

    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_no_provided_route() {
        block_on_all(async {
            let mut test_basics = setup_test_basics_internal(TestConfiguration {
                cluster_mode: ClusterMode::Enabled,
                shared_server: true,
                ..Default::default()
            })
            .await;

            let mut cmd = redis::cmd("INFO");
            cmd.arg("REPLICATION");
            let info = test_basics.client.send_command(&cmd, None).await.unwrap();
            let info = redis::from_owned_redis_value::<HashMap<String, String>>(info).unwrap();
            let (primaries, replicas) = count_primaries_and_replicas(info);
            assert_eq!(primaries, 3);
            assert_eq!(replicas, 0);
        });
    }

    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_to_all_primaries() {
        block_on_all(async {
            let mut test_basics = setup_test_basics_internal(TestConfiguration {
                cluster_mode: ClusterMode::Enabled,
                shared_server: true,
                ..Default::default()
            })
            .await;

            let mut cmd = redis::cmd("INFO");
            cmd.arg("REPLICATION");
            let info = test_basics
                .client
                .send_command(
                    &cmd,
                    Some(RoutingInfo::MultiNode((
                        MultipleNodeRoutingInfo::AllMasters,
                        None,
                    ))),
                )
                .await
                .unwrap();
            let info = redis::from_owned_redis_value::<HashMap<String, String>>(info).unwrap();
            let (primaries, replicas) = count_primaries_and_replicas(info);
            assert_eq!(primaries, 3);
            assert_eq!(replicas, 0);
        });
    }

    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_to_all_nodes() {
        block_on_all(async {
            let mut test_basics = setup_test_basics_internal(TestConfiguration {
                cluster_mode: ClusterMode::Enabled,
                shared_server: true,
                ..Default::default()
            })
            .await;

            let mut cmd = redis::cmd("INFO");
            cmd.arg("REPLICATION");
            let info = test_basics
                .client
                .send_command(
                    &cmd,
                    Some(RoutingInfo::MultiNode((
                        MultipleNodeRoutingInfo::AllNodes,
                        None,
                    ))),
                )
                .await
                .unwrap();
            let info = redis::from_owned_redis_value::<HashMap<String, String>>(info).unwrap();
            let (primaries, replicas) = count_primaries_and_replicas(info);
            assert_eq!(primaries, 3);
            assert_eq!(replicas, 3);
        });
    }

    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_by_slot_to_primary() {
        block_on_all(async {
            let mut test_basics = setup_test_basics_internal(TestConfiguration {
                cluster_mode: ClusterMode::Enabled,
                shared_server: true,
                ..Default::default()
            })
            .await;

            let mut cmd = redis::cmd("INFO");
            cmd.arg("REPLICATION");
            let info = test_basics
                .client
                .send_command(
                    &cmd,
                    Some(RoutingInfo::SingleNode(
                        SingleNodeRoutingInfo::SpecificNode(Route::new(0, SlotAddr::Master)),
                    )),
                )
                .await
                .unwrap();
            let info = redis::from_owned_redis_value::<String>(info).unwrap();
            let (primaries, replicas) = count_primary_or_replica(&info);
            assert_eq!(primaries, 1);
            assert_eq!(replicas, 0);
        });
    }

    #[cfg(feature = "valkey-GTE-7-2")]
    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_by_slot_to_replica_if_read_from_az_affinity_configuration_allows(// #[values(4)] replica_num: u16,
    ) {
        use std::{thread::sleep, time::Duration};
        // let cluster = TestClusterContext::new(9, 2);

        block_on_all(async {
            let replica_num: u16 = 2;
            let primaries_num: u16 = 3;
            let az: String = "us-east-1a".to_string();

            let mut test_basics = setup_cluster_with_replicas(
                TestConfiguration {
                    cluster_mode: ClusterMode::Enabled,
                    shared_server: false,
                    read_from: Some(ReadFrom::AZAffinity),
                    client_az: Some(az.clone()),
                    ..Default::default()
                },
                replica_num,
                primaries_num,
            )
            .await;

            let mut cmd = redis::cmd("CONFIG");
            cmd.arg(&["SET", "availability-zone", &az]);

            // Set half of the replicas in the same client_az
            let replicas_in_same_az = replica_num / 2;
            for _ in 0..replicas_in_same_az {
                test_basics
                    .client
                    .send_command(
                        &cmd,
                        // Some(RoutingInfo::SingleNode(
                        //     SingleNodeRoutingInfo::SpecificNode(Route::new(
                        //         12182, // foo key is mapping to 12182 slot
                        //         SlotAddr::ReplicaOptional,
                        //     )),
                        // )),
                        Some(RoutingInfo::MultiNode((
                            MultipleNodeRoutingInfo::AllNodes,
                            None,
                        ))),
                    )
                    .await
                    .unwrap();
            }
            sleep(Duration::from_secs(10));

            let mut cmd = redis::cmd("GET");
            cmd.arg(&["foo"]);
            let n = 3;

            // let mut connection = test_basics.async_connection(None).await;

            // Each replica will return the value of foo n times
            for _ in 0..(n * replica_num) {
                // cmd.query_async(&mut connection).await;
                test_basics
                    .client
                    .send_command(
                        &cmd,
                        Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random)),
                    )
                    .await
                    .unwrap();
            }
            sleep(Duration::from_secs(200000000000000000));
            let mut cmd = redis::cmd("INFO");
            cmd.arg("ALL");
            let info = test_basics
                .client
                .send_command(
                    &cmd,
                    Some(RoutingInfo::MultiNode((
                        MultipleNodeRoutingInfo::AllNodes,
                        None,
                    ))),
                )
                .await
                .unwrap();

            let info_result =
                redis::from_owned_redis_value::<HashMap<String, String>>(info).unwrap();
            println!("{:?}", info_result.values());
            let get_cmdstat = format!("cmdstat_get:calls={}", n);
            let client_az = format!("availability_zone:{}", az);

            let matching_entries_count = info_result
                .values()
                .filter(|value| value.contains(&get_cmdstat) && value.contains(&client_az))
                .count();

            assert_eq!(
                (matching_entries_count.try_into() as Result<u16, _>).unwrap(),
                replica_num,
                "Test failed: expected exactly '{}' entries with '{}' and '{}', found {}",
                replica_num.to_string(),
                get_cmdstat,
                client_az,
                matching_entries_count
            );
        })
    }

    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_by_slot_to_replica_if_read_from_replica_configuration_allows() {
        block_on_all(async {
            let mut test_basics = setup_test_basics_internal(TestConfiguration {
                cluster_mode: ClusterMode::Enabled,
                shared_server: true,
                read_from: Some(ReadFrom::PreferReplica),
                ..Default::default()
            })
            .await;

            let mut cmd = redis::cmd("INFO");
            cmd.arg("REPLICATION");
            let info = test_basics
                .client
                .send_command(
                    &cmd,
                    Some(RoutingInfo::SingleNode(
                        SingleNodeRoutingInfo::SpecificNode(Route::new(
                            0,
                            SlotAddr::ReplicaOptional,
                        )),
                    )),
                )
                .await
                .unwrap();
            let info = redis::from_owned_redis_value::<String>(info).unwrap();
            let (primaries, replicas) = count_primary_or_replica(&info);
            assert_eq!(primaries, 0);
            assert_eq!(replicas, 1);
        });
    }

    #[rstest]
    #[timeout(SHORT_CLUSTER_TEST_TIMEOUT)]
    fn test_send_routing_by_slot_to_replica_override_read_from_replica_configuration() {
        block_on_all(async {
            let mut test_basics = setup_test_basics_internal(TestConfiguration {
                cluster_mode: ClusterMode::Enabled,
                shared_server: true,
                read_from: Some(ReadFrom::Primary),
                ..Default::default()
            })
            .await;

            let mut cmd = redis::cmd("INFO");
            cmd.arg("REPLICATION");
            let info = test_basics
                .client
                .send_command(
                    &cmd,
                    Some(RoutingInfo::SingleNode(
                        SingleNodeRoutingInfo::SpecificNode(Route::new(
                            0,
                            SlotAddr::ReplicaRequired,
                        )),
                    )),
                )
                .await
                .unwrap();
            let info = redis::from_owned_redis_value::<String>(info).unwrap();
            let (primaries, replicas) = count_primary_or_replica(&info);
            assert_eq!(primaries, 0);
            assert_eq!(replicas, 1);
        });
    }
}
