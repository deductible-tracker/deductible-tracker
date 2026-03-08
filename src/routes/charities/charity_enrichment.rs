pub(super) fn normalize_ein(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
}

pub(super) fn normalize_i64_ein(value: i64) -> String {
    format!("{:09}", value)
}

// Maximum allowed length for cleaned optional strings to avoid unbounded allocations.
const MAX_CLEAN_STRING_LEN: usize = 1024;

pub(super) fn clean_opt_string(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        if s.len() > MAX_CLEAN_STRING_LEN {
            // Reject overly long input to bound allocation size.
            return None;
        }
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn map_deductibility(code: Option<i64>) -> Option<String> {
    match code {
        Some(1) => Some("Contributions are deductible".to_string()),
        Some(2) => Some("Contributions are not deductible".to_string()),
        Some(4) => Some("Contributions are deductible by treaty".to_string()),
        Some(other) => Some(format!("Code {}", other)),
        None => None,
    }
}

pub(super) fn map_deductibility_from_exempt_status(exempt_status_code: Option<i64>) -> Option<String> {
    match exempt_status_code {
        Some(1) => Some("Deductible".to_string()),
        Some(2) | Some(3) | Some(4) => Some("May not be deductible".to_string()),
        Some(other) => Some(format!("Status {}", other)),
        None => None,
    }
}

pub(super) fn map_nonprofit_type(subsection_code: Option<i64>) -> Option<String> {
    subsection_code.and_then(map_tax_section).map(|s| s.to_string())
}

pub(super) fn map_tax_section(subsection_code: i64) -> Option<&'static str> {
    match subsection_code {
        2 => Some("501(c)(2)"),
        3 => Some("501(c)(3)"),
        4 => Some("501(c)(4)"),
        5 => Some("501(c)(5)"),
        6 => Some("501(c)(6)"),
        7 => Some("501(c)(7)"),
        8 => Some("501(c)(8)"),
        9 => Some("501(c)(9)"),
        10 => Some("501(c)(10)"),
        11 => Some("501(c)(11)"),
        12 => Some("501(c)(12)"),
        13 => Some("501(c)(13)"),
        14 => Some("501(c)(14)"),
        15 => Some("501(c)(15)"),
        16 => Some("501(c)(16)"),
        17 => Some("501(c)(17)"),
        18 => Some("501(c)(18)"),
        19 => Some("501(c)(19)"),
        21 => Some("501(c)(21)"),
        22 => Some("501(c)(22)"),
        23 => Some("501(c)(23)"),
        25 => Some("501(c)(25)"),
        26 => Some("501(c)(26)"),
        27 => Some("501(c)(27)"),
        28 => Some("501(c)(28)"),
        92 => Some("4947(a)(1)"),
        _ => None,
    }
}

pub(super) fn map_category_from_ntee(ntee_code: Option<&str>) -> Option<String> {
    let letter = ntee_code
        .and_then(|code| code.chars().next())
        .map(|c| c.to_ascii_uppercase());

    let category = match letter {
        Some('A') => "Arts, Culture & Humanities",
        Some('B') => "Education",
        Some('C') | Some('D') => "Environment and Animals",
        Some('E') | Some('F') | Some('G') | Some('H') => "Health",
        Some('I') | Some('J') | Some('K') | Some('L') | Some('M') | Some('N') | Some('O') | Some('P') => "Human Services",
        Some('Q') => "International, Foreign Affairs",
        Some('R') | Some('S') | Some('T') | Some('U') | Some('V') | Some('W') => "Public, Societal Benefit",
        Some('X') => "Religion Related",
        Some('Y') => "Mutual/Membership Benefit",
        _ => "Unknown, Unclassified",
    };
    Some(category.to_string())
}

pub(super) fn map_exempt_status_label(code: Option<i64>) -> Option<&'static str> {
    match code {
        Some(1) => Some("Active"),
        Some(2) => Some("Exempt"),
        Some(3) => Some("Revoked"),
        Some(4) => Some("Terminated"),
        _ => None,
    }
}

pub(super) fn map_foundation_label(code: Option<i64>) -> Option<String> {
    match code {
        Some(0) => Some("Non-501(c)(3)".to_string()),
        Some(2) => Some("Private Operating (tax-exempt investment income)".to_string()),
        Some(3) => Some("Private Operating".to_string()),
        Some(4) => Some("Private Non-Operating".to_string()),
        Some(9) => Some("Suspense".to_string()),
        Some(10) => Some("Church".to_string()),
        Some(11) => Some("School".to_string()),
        Some(12) => Some("Hospital/Medical Research".to_string()),
        Some(13) => Some("Gov-Owned College/University Support".to_string()),
        Some(14) => Some("Governmental Unit".to_string()),
        Some(15) => Some("Public Support (Gov/Public)".to_string()),
        Some(16) => Some("509(a)(2)".to_string()),
        Some(17) => Some("509(a)(3) Supporting Org".to_string()),
        Some(18) => Some("509(a)(4) Public Safety Testing".to_string()),
        Some(other) => Some(format!("Foundation {}", other)),
        None => None,
    }
}

pub(super) fn derive_status(exempt_status_code: Option<i64>) -> Option<String> {
    if let Some(status_label) = map_exempt_status_label(exempt_status_code) {
        Some(status_label.to_string())
    } else {
        exempt_status_code.map(|code| format!("Status {}", code))
    }
}