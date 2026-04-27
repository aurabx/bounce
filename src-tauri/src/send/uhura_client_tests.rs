#[cfg(test)]
mod tests {
    use crate::send::uhura_client::{extract_boundary, split_multipart_parts};

    #[test]
    fn test_extract_boundary_quoted() {
        let ct = "multipart/related; type=\"application/dicom\"; boundary=\"abc123\"";
        assert_eq!(extract_boundary(ct).as_deref(), Some("abc123"));
    }

    #[test]
    fn test_extract_boundary_unquoted() {
        let ct = "multipart/related; boundary=xyzboundary";
        assert_eq!(extract_boundary(ct).as_deref(), Some("xyzboundary"));
    }

    #[test]
    fn test_extract_boundary_with_other_params_first() {
        let ct = "multipart/related; type=application/dicom; boundary=\"---bdry---\"";
        assert_eq!(extract_boundary(ct).as_deref(), Some("---bdry---"));
    }

    #[test]
    fn test_extract_boundary_missing() {
        let ct = "multipart/related; type=application/dicom";
        assert!(extract_boundary(ct).is_none());
    }

    #[test]
    fn test_extract_boundary_empty_value() {
        let ct = "multipart/related; boundary=";
        assert!(extract_boundary(ct).is_none());
    }

    #[test]
    fn test_split_multipart_two_parts() {
        let body = b"--BOUNDARY\r\nContent-Type: application/dicom\r\n\r\nFIRST_PART_BYTES\r\n--BOUNDARY\r\nContent-Type: application/dicom\r\n\r\nSECOND\r\n--BOUNDARY--\r\n";
        let parts = split_multipart_parts(body, "BOUNDARY").unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], b"FIRST_PART_BYTES");
        assert_eq!(parts[1], b"SECOND");
    }

    #[test]
    fn test_split_multipart_with_preamble() {
        // Per RFC 2046, content before the first boundary is a discardable
        // preamble; the parser should skip it without complaint.
        let body = b"This is a preamble that should be skipped\r\n--B\r\nContent-Type: application/dicom\r\n\r\nbody\r\n--B--\r\n";
        let parts = split_multipart_parts(body, "B").unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], b"body");
    }

    #[test]
    fn test_split_multipart_single_part() {
        let body = b"--B\r\nContent-Type: application/dicom\r\n\r\nonly\r\n--B--\r\n";
        let parts = split_multipart_parts(body, "B").unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], b"only");
    }

    #[test]
    fn test_split_multipart_missing_initial_boundary_errors() {
        let body = b"no boundary anywhere";
        let result = split_multipart_parts(body, "ABSENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_split_multipart_truncated_after_boundary_errors() {
        let body = b"--B";
        let result = split_multipart_parts(body, "B");
        assert!(result.is_err());
    }

    #[test]
    fn test_split_multipart_preserves_binary_content() {
        // DICOM bytes are arbitrary binary; the parser must not assume UTF-8.
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(b"--BDY\r\nContent-Type: application/dicom\r\n\r\n");
        body.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0xFF]);
        body.extend_from_slice(b"\r\n--BDY--\r\n");
        let parts = split_multipart_parts(&body, "BDY").unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0xFF]);
    }
}
