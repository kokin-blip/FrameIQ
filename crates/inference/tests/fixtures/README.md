# Local HTTPS test material

The certificate and private key here are disposable localhost test fixtures, not application credentials. The certificate is self-signed and trusted only by the private unit-test transport. Production downloads do not load this certificate or permit localhost. The pair is included so the TLS tests run without OpenSSL or Python on Windows CI.

Regenerate both together with a localhost SAN, `CA:FALSE`, and `serverAuth` when the certificate expires. These files are not packaged with FrameIQ.
