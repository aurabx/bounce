use crate::log_info;

pub(crate) fn describe_request(command_field: u16) -> &'static str {
    match command_field {
        0x0001 => "C-STORE-RQ",
        0x0010 => "C-GET-RQ",
        0x0020 => "C-FIND-RQ",
        0x0021 => "C-MOVE-RQ",
        0x0030 => "C-ECHO-RQ",
        0x0FFF => "C-CANCEL-RQ",
        _ => "DIMSE-RQ",
    }
}

pub(crate) fn format_optional_str(value: Option<&str>) -> String {
    value
        .filter(|inner| !inner.is_empty())
        .unwrap_or("n/a")
        .to_string()
}

pub(crate) fn format_optional_u16(value: Option<u16>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

pub(crate) fn log_scp_request(
    peer: &str,
    message: &str,
    pc_id: u8,
    msg_id: u16,
    details: &[(&str, String)],
) {
    log_info!(
        "DIMSE SCP <- {} from {} pc_id={} msg_id={}{}",
        message,
        peer,
        pc_id,
        msg_id,
        format_details(details),
    );
}

pub(crate) fn log_scp_response(
    peer: &str,
    message: &str,
    pc_id: u8,
    msg_id: u16,
    status: u16,
    details: &[(&str, String)],
) {
    log_info!(
        "DIMSE SCP -> {} to {} pc_id={} msg_id={} status=0x{:04X}{}",
        message,
        peer,
        pc_id,
        msg_id,
        status,
        format_details(details),
    );
}

pub(crate) fn log_scp_payload(message: &str, details: &[(&str, String)]) {
    log_info!(
        "DIMSE SCP payload <- {}{}",
        message,
        format_details(details)
    );
}

pub(crate) fn log_scu_request(
    target_ae: &str,
    addr: &str,
    message: &str,
    msg_id: u16,
    details: &[(&str, String)],
) {
    log_info!(
        "DIMSE SCU -> {} to {}@{} msg_id={}{}",
        message,
        target_ae,
        addr,
        msg_id,
        format_details(details),
    );
}

pub(crate) fn log_scu_response(
    target_ae: &str,
    addr: &str,
    message: &str,
    msg_id: Option<u16>,
    status: u16,
    details: &[(&str, String)],
) {
    log_info!(
        "DIMSE SCU <- {} from {}@{} msg_id={} status=0x{:04X}{}",
        message,
        target_ae,
        addr,
        format_optional_u16(msg_id),
        status,
        format_details(details),
    );
}

pub(crate) fn log_scu_data(target_ae: &str, addr: &str, message: &str, len: usize, pc_id: u8) {
    log_info!(
        "DIMSE SCU <- {} data from {}@{} len={} pc_id={}",
        message,
        target_ae,
        addr,
        len,
        pc_id,
    );
}

fn format_details(details: &[(&str, String)]) -> String {
    if details.is_empty() {
        return String::new();
    }

    let suffix = details
        .iter()
        .map(|(key, value)| format!("{}={}", key, value))
        .collect::<Vec<String>>()
        .join(" ");

    format!(" {}", suffix)
}
