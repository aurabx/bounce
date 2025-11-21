#[cfg(test)]
mod tests {
    use crate::db::database::Database;
    use crate::receiver::dicom_server::DICOMServer;
    use crate::store::config::Config;
    use dicom::core::{DataElement, VR};
    use dicom::dicom_value;
    use dicom::dictionary_std::tags;
    use dicom::encoding::TransferSyntaxIndex;
    use dicom::object::InMemDicomObject;
    use dicom_ul::association::ClientAssociationOptions;
    use dicom_ul::pdu::Pdu;
    use sqlx::SqlitePool;
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    async fn setup() -> (DICOMServer, TempDir) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db = Database::new_for_test(pool).await;
        let temp_dir = TempDir::new().unwrap();

        let config = Config {
            api_key: "test_key".to_string(),
            port: 0, // Not used in run_store_sync
            ip_address: "127.0.0.1".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: temp_dir.path().to_string_lossy().to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let server = DICOMServer::new_for_test(config, db);
        (server, temp_dir)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cecho_handling() {
        let (server, _temp_dir) = setup().await;

        // Create a listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn client
        let client_handle = tokio::spawn(async move {
            let mut association = ClientAssociationOptions::new()
                .with_abstract_syntax("1.2.840.10008.1.1") // Verification SOP Class
                .establish_async(&addr)
                .await
                .expect("Failed to establish association");

            // Send C-ECHO
            let pc = association.presentation_contexts().first().unwrap().id;

            // Construct C-ECHO-RQ Command Object
            let command = InMemDicomObject::command_from_element_iter([
                DataElement::new(
                    tags::AFFECTED_SOP_CLASS_UID,
                    VR::UI,
                    dicom_value!(Str, "1.2.840.10008.1.1"),
                ),
                DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x0030])), // C-ECHO-RQ
                DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [1])),
                DataElement::new(
                    tags::COMMAND_DATA_SET_TYPE,
                    VR::US,
                    dicom_value!(U16, [0x0101]),
                ), // No dataset
            ]);

            let mut command_data = Vec::new();
            // Commands are always Implicit VR Little Endian
            let ts = dicom::transfer_syntax::TransferSyntaxRegistry
                .get("1.2.840.10008.1.2")
                .expect("Implicit VR LE not found");

            command
                .write_dataset_with_ts(&mut command_data, ts)
                .expect("Failed to write command");

            let msg = dicom_ul::pdu::PDataValue {
                presentation_context_id: pc,
                value_type: dicom_ul::pdu::PDataValueType::Command,
                is_last: true,
                data: command_data,
            };

            association
                .send(&Pdu::PData { data: vec![msg] })
                .await
                .expect("Failed to send C-ECHO");

            // Receive response
            let pdu = association
                .receive()
                .await
                .expect("Failed to receive response");
            match pdu {
                Pdu::PData { data } => {
                    // Basic validation that we got a response
                    assert!(!data.is_empty());
                }
                _ => panic!("Expected PData response"),
            }

            association.release().await.expect("Failed to release");
        });

        // Accept connection
        let (stream, _) = listener.accept().await.unwrap();
        let out_dir = std::path::PathBuf::from(_temp_dir.path());

        // Convert tokio stream to std stream because run_store_sync takes std::net::TcpStream
        // Wait, run_store_sync signature:
        // pub async fn run_store_sync(&self, scu_stream: TcpStream, out_dir: &PathBuf)
        // The signature says TcpStream. Which one?
        // In dicom_server.rs: use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
        // So it expects std::net::TcpStream.

        let std_stream = stream.into_std().unwrap();
        std_stream.set_nonblocking(false).unwrap();

        let server_clone = server.clone();
        let out_dir_clone = out_dir.clone();

        // Run the server handler in a blocking task to avoid blocking the async runtime
        let server_handle = tokio::task::spawn_blocking(move || {
            // We need a runtime to block on the async fn because it uses async internally?
            // Actually, run_store_sync is async fn, so we must run it on a runtime.
            // But since it blocks, we can't run it on the current thread if single threaded.
            // With 'multi_thread', it should be fine to call it, but to be safe:

            // The problem is that run_store_sync uses synchronous blocking I/O (dicom-ul standard)
            // inside an async function.
            // When we await it, we block the executor thread.

            // So we spawn a new runtime or block_on?
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                server_clone
                    .run_store_sync(std_stream, &out_dir_clone)
                    .await
                    .unwrap();
            });
        });

        client_handle.await.unwrap();
        server_handle.await.unwrap();
    }
}
