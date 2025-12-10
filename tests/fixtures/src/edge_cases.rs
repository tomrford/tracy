/// REQ-60: Multiple on one line REQ-61 and REQ-62
fn multi() {}

/// REQ-63
/// REQ-63: Same slug different lines - should appear twice
fn duplicate_lines() {}

/// REQ-64 REQ-64 REQ-64: Same slug same line - should dedupe to one
fn duplicate_same_line() {}

/*! REQ-65: Inner block doc */

/// [REQ-66] (REQ-67) {REQ-68}: Special chars around slugs
fn special() {}

/// Trailing ref: REQ-69
fn trailing() {}

/// https://tracker.example.com/REQ-70
fn in_url() {}

/// REQ-007: Leading zeros preserved
fn leading_zeros() {}
