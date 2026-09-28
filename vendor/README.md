# Patched dependencies

`hyper-util` is vendored from crates.io version 0.1.20, under its original MIT
license (see `hyper-util/LICENSE`). Upstream source and tests are retained.

The local patch exports `TunnelError` and preserves the destination host, port,
HTTP status and raw reason phrase of rejected CONNECT responses. It also waits
for a complete status line before parsing fragmented proxy responses, and
distinguishes an empty response from a truncated response on EOF. This allows
the compiler to reproduce Elm 0.19.1 proxy diagnostics without guessing a status
from a generic transport error. The Cargo crates.io patch and standalone export
both include this source directory.

Rejected statuses are reported only after the response headers complete; EOF
after a status line still remains an incomplete response error.

Malformed status lines retain their raw bytes for `InvalidStatusLine` diagnostics.
The status parser accepts numeric HTTP versions and space-separated integer
statuses as observed with Elm 0.19.1. Behavioral reference:
https://github.com/snoyberg/http-client/blob/http-client-0.6.4/http-client/Network/HTTP/Client/Headers.hs
(`parseStatus` and `parseVersion`). The CONNECT reader skips complete 100 Continue responses while preserving any
following response already buffered, matching the reference's special handling
of 100 (other 1xx statuses remain final here). The reader accepts LF and CRLF line endings (including mixed endings) and
skips up to three leading blank lines per status. Headers are consumed line by line. The 100th line containing a colon triggers
OverlongHeaders except within interim 100 responses. Before reading another
8192-byte chunk, an incomplete line above 4096 bytes triggers the same error.
The parser uses a bounded 12288-byte buffer instead of buffering the total
header block. As in the reference, complete long lines can be accepted depending
on receive boundaries; this is not a fixed total-response size limit. Line handling reference:
https://github.com/snoyberg/http-client/blob/http-client-0.6.4/http-client/Network/HTTP/Client/Connection.hs

`tower-http` is vendored from crates.io version 0.6.11 under its original MIT
license (see `tower-http/LICENSE`), with upstream sources and tests retained.
The patch adds a `decompression::GzipDecoded` response extension
when gzip decoding is selected. This preserves the encoding decision after the
middleware removes Content-Encoding and Content-Length. The compiler uses it
to scope Elm-compatible gzip EOF tolerance and diagnostics without treating HTTP
framing errors or TLS truncation as gzip decoder EOF. The gzip decoder also
ignores data frames left after its first completed member, matching Elm's
http-client behavior. This is selected through a decorator constant whose default
remains false for every other codec. Underlying body errors still propagate;
JSON, ZIP and archive hash validation remain the compiler's responsibility.

`compression-codecs` is vendored from crates.io 0.4.38 under its original
MIT OR Apache-2.0 license, retaining upstream sources, tests and license files.
The gzip header parser now takes the parsed header only after testing FHCRC
and reading/checking its two bytes. Previously it reset the flags before testing
FHCRC, feeding the checksum bytes to DEFLATE instead. The local regression
covers complete and fragmented valid headers and a corrupted header checksum.
The header parser also rejects the three reserved flag bits, matching the
reference inflater instead of silently ignoring them. A separate regression
checks each reserved bit independently.

On Linux, hyper-util now retains the descriptor of a refused TCP connection in
its exported TcpConnectError cause before Tokio consumes the socket. Other
connection errors and platforms retain their original error chain. The compiler
uses this typed descriptor and the ConnectionRefused kind to render the observed
Haskell socket diagnostic; no descriptor is guessed or copied from a fixture.

The opt-in `elm-no-proxy` feature changes environment-derived exclusions to
Haskell http-client's textual domain-suffix rules. It preserves IPv6 brackets,
does not interpret CIDR ranges, removes only leading ASCII spaces, and gives
`no_proxy` precedence over `NO_PROXY`. Explicit matcher builders retain their
original rules and tests. When neither variable exists, system exclusions are
left intact. Reference: https://github.com/snoyberg/http-client/blob/http-client-0.6.4/http-client/Network/HTTP/Proxy.hs
(`envHelper`, `domainSuffixes`, `hasDomainSuffixIn`).

Environment lookup also recognizes mixed-case spellings of NO_PROXY. An exact
lowercase key takes precedence, including an empty value; otherwise the last
case-insensitive match in the environment snapshot is used, matching the
reference's case-folded map. Unrelated non-Unicode environment values are not
converted or inspected.

The compiler now enables `elm-proxy-environment`, which includes `elm-no-proxy`.
For environment-derived proxies it selects the protocol-specific HTTP/HTTPS
variable using the same lowercase-first, case-insensitive fallback lookup and
ignores ALL_PROXY. Explicit builders retain their existing all-proxy behavior.
CGI handling and platform configuration fallbacks are unchanged by this patch.

For those environment-derived proxies, authentication requires a literal colon
in userinfo before percent decoding. The decoded UTF-8 characters are packed
as Char8 bytes before Base64 encoding, matching the reference
`extractBasicAuthInfo` (`ByteString.Char8.pack . unEscapeString`):
https://github.com/snoyberg/http-client/blob/http-client-0.6.4/http-client/Network/HTTP/Client/Request.hs
A separate builder flag scopes this behavior to environment discovery; explicit
builders and SOCKS authentication retain upstream semantics. Generated Basic
auth headers remain marked sensitive. Tests cover Unicode, invalid UTF-8, absent
passwords and empty usernames, with differential wire-level assertions.

The environment credential decoder also preserves network-uri's handling of
escaped invalid UTF-8: it consumes available continuation escapes before
rejecting a scalar, including obsolete five/six-byte forms, and rejects U+FFFE
and U+FFFF. Interrupted sequences leave the first non-continuation token for
the next iteration. This differs from Rust's `decode_utf8_lossy` replacement
granularity. The behavior is restricted to proxy userinfo and verified against
the official compiler's wire headers. Algorithm reference:
https://github.com/haskell/network-uri/blob/master/Network/URI.hs
(`unEscapeString` and `unEscapeUtf8`).

`rustls` is vendored from crates.io 0.23.45 with its original Apache-2.0,
ISC and MIT license files, source and tests retained. The opt-in `elm-name-first`
feature runs `verify_server_name` immediately after parsing the leaf certificate,
before the existing trust-chain verification. Both checks must still succeed;
signature, validity, trust, revocation and TLS handshake checks are retained.
Without the feature, the upstream order is unchanged. This matches observed
Elm diagnostics when a name mismatch coexists with expiration or an invalid
certificate signature. No trust-store or reqwest configuration was replaced.
