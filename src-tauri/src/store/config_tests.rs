#[cfg(test)]
mod tests {
    use super::super::config::Config;

    #[test]
    fn test_region_from_api_key_valid() {
        let config = Config {
            api_key: "aura_au_bounce_user_ADASDASDASDASDASDA_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let region = config.region_from_api_key();
        assert_eq!(region, Some("au".to_string()));
    }

    #[test]
    fn test_region_from_api_key_us_region() {
        let config = Config {
            api_key: "aura_us_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let region = config.region_from_api_key();
        assert_eq!(region, Some("us".to_string()));
    }

    #[test]
    fn test_region_from_api_key_invalid_format() {
        let config = Config {
            api_key: "invalid_key_format".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let region = config.region_from_api_key();
        assert_eq!(region, None);
    }

    #[test]
    fn test_region_from_api_key_empty() {
        let config = Config {
            api_key: "".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let region = config.region_from_api_key();
        assert_eq!(region, None);
    }

    #[test]
    fn test_mode_from_api_key_production() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let mode = config.mode_from_api_key();
        assert_eq!(mode, Some("production".to_string()));
    }

    #[test]
    fn test_mode_from_api_key_staging() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_staging".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let mode = config.mode_from_api_key();
        assert_eq!(mode, Some("staging".to_string()));
    }

    #[test]
    fn test_mode_from_api_key_dev() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_dev".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let mode = config.mode_from_api_key();
        assert_eq!(mode, Some("dev".to_string()));
    }

    #[test]
    fn test_mode_from_api_key_local() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_local".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let mode = config.mode_from_api_key();
        assert_eq!(mode, Some("local".to_string()));
    }

    #[test]
    fn test_mode_from_api_key_invalid_defaults_to_production() {
        let config = Config {
            api_key: "invalid_key".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let mode = config.mode_from_api_key();
        assert_eq!(mode, Some("production".to_string()));
    }

    #[test]
    fn test_get_api_endpoint_production_au() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let endpoint = config.get_api_endpoint();
        assert_eq!(endpoint, "https://au.aurabox.app");
    }

    #[test]
    fn test_get_api_endpoint_production_us() {
        let config = Config {
            api_key: "aura_us_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let endpoint = config.get_api_endpoint();
        assert_eq!(endpoint, "https://us.aurabox.app");
    }

    #[test]
    fn test_get_api_endpoint_staging() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_staging".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let endpoint = config.get_api_endpoint();
        assert_eq!(
            endpoint,
            "https://staging-5em2ouy-pghszvpk65pns.au.platformsh.site"
        );
    }

    #[test]
    fn test_get_api_endpoint_dev() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_dev".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let endpoint = config.get_api_endpoint();
        assert_eq!(endpoint, "https://dev-54ta5gq-pghszvpk65pns.au.platformsh.site");
    }

    #[test]
    fn test_get_api_endpoint_local() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_local".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "./tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let endpoint = config.get_api_endpoint();
        assert_eq!(endpoint, "https://aura.lndo.site");
    }

    #[test]
    fn test_resolve_study_path() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "/tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let study_uid = "1.2.3.4.5".to_string();
        let path = config.resolve_study_path(&study_uid);

        assert_eq!(path.to_str().unwrap(), "/tmp/dicom_storage/1.2.3.4.5");
    }

    #[test]
    fn test_resolve_study_path_with_null_terminator() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "/tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let study_uid = "1.2.3.4.5\0\0".to_string();
        let path = config.resolve_study_path(&study_uid);

        // Should trim null terminators
        assert_eq!(path.to_str().unwrap(), "/tmp/dicom_storage/1.2.3.4.5");
    }

    #[test]
    fn test_resolve_metadata_path() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "/tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let study_uid = "1.2.3.4.5".to_string();
        let path = config.resolve_metadata_path(&study_uid);

        assert_eq!(path.to_str().unwrap(), "/tmp/dicom_storage/1.2.3.4.5.json");
    }

    #[test]
    fn test_resolve_metadata_path_with_null_terminator() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "/tmp/dicom_storage".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        let study_uid = "1.2.3.4.5\0".to_string();
        let path = config.resolve_metadata_path(&study_uid);

        assert_eq!(path.to_str().unwrap(), "/tmp/dicom_storage/1.2.3.4.5.json");
    }

    #[test]
    fn test_get_base_dir() {
        let config = Config {
            api_key: "aura_au_bounce_user_TOKEN_production".to_string(),
            port: 9090,
            ip_address: "0.0.0.0".to_string(),
            ae_title: "BOUNCE".to_string(),
            base_dir: "/custom/path".to_string(),
            delete_after_success: "no".to_string(),
            send_logs: "no".to_string(),
        };

        assert_eq!(config.get_base_dir(), "/custom/path");
    }
}
