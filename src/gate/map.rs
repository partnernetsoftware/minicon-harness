//! The routing map: which upstream outcome maps to which verdict, which axis
//! cools, and for how long. Pure data plus a pure function, so the reader
//! recomputes the same table from the contract.
//!
//! This is the `PRD_01_27` "Fallback" table as code. A non-429 4xx never
//! falls back -- the next lane would answer identically.

use super::evidence::{Axis, Verdict};

/// What the router does after one attempt settles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Disposition {
    /// The recorded verdict.
    pub verdict: Verdict,
    /// Whether another candidate may be tried.
    pub fallback: bool,
    /// The axis to cool, when an attempt settles in a way that names one.
    pub cool: Option<(Axis, u64)>,
    /// The `trigger_reason` the next attempt carries.
    pub reason: &'static str,
}

impl Disposition {
    const fn new(verdict: Verdict, fallback: bool, cool: Option<(Axis, u64)>, reason: &'static str) -> Self {
        Self {
            verdict,
            fallback,
            cool,
            reason,
        }
    }
}

/// Cool durations, in milliseconds, from the contract table.
pub mod cool_ms {
    /// An upstream 429 cools the credential for this long.
    pub const CREDENTIAL_429: u64 = 60_000;
    /// An upstream 5xx cools the egress for this long.
    pub const EGRESS_5XX: u64 = 30_000;
    /// An upstream 401/403 cools the credential for this long.
    pub const CREDENTIAL_AUTH: u64 = 300_000;
    /// A connect/TLS/tunnel failure cools the egress for this long.
    pub const EGRESS_CONNECT: u64 = 60_000;
    /// A proxy 407 cools the egress for this long.
    pub const EGRESS_407: u64 = 300_000;
    /// An over-bound body cools the lane for this long.
    pub const LANE_BODY: u64 = 30_000;
}

/// Classic 429: rotate the credential, keep the pipe.
pub const UPSTREAM_429: Disposition = Disposition::new(
    Verdict::Refused,
    true,
    Some((Axis::Credential, cool_ms::CREDENTIAL_429)),
    "upstream_429",
);

/// Upstream 5xx: keep the credential, rotate the pipe.
pub const UPSTREAM_5XX: Disposition = Disposition::new(
    Verdict::Refused,
    true,
    Some((Axis::Egress, cool_ms::EGRESS_5XX)),
    "upstream_5xx",
);

/// Upstream 401/403: the credential is wrong; another lane will not fix it.
pub const UPSTREAM_AUTH: Disposition = Disposition::new(
    Verdict::Refused,
    false,
    Some((Axis::Credential, cool_ms::CREDENTIAL_AUTH)),
    "upstream_unauthorized",
);

/// Upstream 400/404/422 and every other non-429 4xx: the request is wrong.
pub const UPSTREAM_REQUEST_ERROR: Disposition = Disposition::new(
    Verdict::Refused,
    false,
    None,
    "upstream_request_error",
);

/// Connect, TLS or tunnel failure.
pub const CONNECT_FAILURE: Disposition = Disposition::new(
    Verdict::Unknown,
    true,
    Some((Axis::Egress, cool_ms::EGRESS_CONNECT)),
    "connect_failure",
);

/// The proxy answered 407: the pipe is broken, not the lane.
pub const PROXY_407: Disposition = Disposition::new(
    Verdict::Unknown,
    false,
    Some((Axis::Egress, cool_ms::EGRESS_407)),
    "proxy_unauthorized",
);

/// The secret could not be read: a configuration error, not a transient one.
pub const SECRET_UNREADABLE: Disposition = Disposition::new(
    Verdict::Unknown,
    false,
    Some((Axis::Credential, 0)),
    "secret_unreadable",
);

/// Success.
pub const PERFORMED: Disposition =
    Disposition::new(Verdict::Performed, false, None, "upstream_2xx");

/// A non-stream body over the response bound.
pub const BODY_OVER_BOUND: Disposition =
    Disposition::new(Verdict::Unknown, true, Some((Axis::Lane, cool_ms::LANE_BODY)), "body_over_bound");

/// The config revision changed mid-request, or a pinned lane vanished.
pub const STALE: Disposition = Disposition::new(Verdict::Stale, false, None, "config_revision_changed");

/// Maps one upstream HTTP status to its disposition.
pub fn classify_status(status: u16) -> Disposition {
    match status {
        200..=299 => PERFORMED,
        429 => UPSTREAM_429,
        500..=599 => UPSTREAM_5XX,
        401 | 403 => UPSTREAM_AUTH,
        _ => UPSTREAM_REQUEST_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contract_table_is_covered_row_by_row() {
        let rows = [
            (200u16, PERFORMED, false),
            (204, PERFORMED, false),
            (429, UPSTREAM_429, true),
            (500, UPSTREAM_5XX, true),
            (502, UPSTREAM_5XX, true),
            (503, UPSTREAM_5XX, true),
            (529, UPSTREAM_5XX, true),
            (401, UPSTREAM_AUTH, false),
            (403, UPSTREAM_AUTH, false),
            (400, UPSTREAM_REQUEST_ERROR, false),
            (404, UPSTREAM_REQUEST_ERROR, false),
            (422, UPSTREAM_REQUEST_ERROR, false),
            (418, UPSTREAM_REQUEST_ERROR, false),
        ];
        for (status, expected, fallback) in rows {
            let got = classify_status(status);
            assert_eq!(got, classify_status(status));
            assert_eq!(got.verdict, expected.verdict, "verdict for {status}");
            assert_eq!(got.fallback, fallback, "fallback for {status}");
            assert_eq!(got.cool, expected.cool, "cooling for {status}");
        }
    }

    #[test]
    fn only_429_cools_the_credential_on_an_error_status() {
        assert_eq!(classify_status(429).cool, Some((Axis::Credential, 60_000)));
        assert_eq!(classify_status(401).cool, Some((Axis::Credential, 300_000)));
        assert_eq!(classify_status(500).cool, Some((Axis::Egress, 30_000)));
        assert_eq!(classify_status(400).cool, None);
    }

    #[test]
    fn non_429_client_errors_never_fall_back() {
        for status in [400u16, 401, 403, 404, 409, 413, 422] {
            assert!(!classify_status(status).fallback, "{status} must not fall back");
        }
    }

    #[test]
    fn proxy_407_cools_the_egress_and_stops() {
        assert_eq!(PROXY_407.cool, Some((Axis::Egress, 300_000)));
        assert!(!PROXY_407.fallback);
        assert_eq!(PROXY_407.verdict, Verdict::Unknown);
    }

    #[test]
    fn an_unreadable_secret_stops_the_chain() {
        assert!(!SECRET_UNREADABLE.fallback);
        assert_eq!(SECRET_UNREADABLE.cool, Some((Axis::Credential, 0)));
    }

    #[test]
    fn connect_failure_rotates_the_pipe() {
        assert_eq!(CONNECT_FAILURE.cool, Some((Axis::Egress, 60_000)));
        assert!(CONNECT_FAILURE.fallback);
        assert_eq!(CONNECT_FAILURE.verdict, Verdict::Unknown);
    }
}
