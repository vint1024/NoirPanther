//! Guards for the hardening this fork adds. Each test here exists because the behaviour it
//! checks was missing at some point and had to be fixed; if an upstream merge drops the fix
//! again, the test fails instead of the problem shipping.
mod headers;
