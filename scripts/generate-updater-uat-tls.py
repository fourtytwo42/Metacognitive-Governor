#!/usr/bin/env python3
import datetime
import hashlib
import ipaddress
import json
import os
import pathlib
import sys

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

VALIDITY_HOURS = 24


def fail(message: str) -> None:
    raise RuntimeError(f"Updater UAT TLS generation rejected: {message}")


def output_path(value: str, label: str) -> pathlib.Path:
    candidate = pathlib.Path(value)
    if not candidate.is_absolute():
        fail(f"{label} must be an absolute new path")
    parent = candidate.parent.resolve(strict=True)
    resolved = parent / candidate.name
    if candidate != resolved or not candidate.name or "\x00" in str(candidate):
        fail(f"{label} must be canonical")
    if candidate.exists() or candidate.is_symlink():
        fail(f"{label} already exists")
    return candidate


def write_new(path: pathlib.Path, payload: bytes, mode: int) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
    finally:
        os.close(descriptor)


if len(sys.argv) != 4:
    fail("usage: generate-updater-uat-tls.py ABSOLUTE_CA_CERT ABSOLUTE_SERVER_CHAIN ABSOLUTE_SERVER_KEY")

ca_path = output_path(sys.argv[1], "CA certificate")
server_path = output_path(sys.argv[2], "server certificate chain")
key_path = output_path(sys.argv[3], "server private key")
if len({ca_path, server_path, key_path}) != 3:
    fail("output paths must be distinct")

now = datetime.datetime.now(datetime.timezone.utc)
not_before = now - datetime.timedelta(minutes=1)
not_after = now + datetime.timedelta(hours=VALIDITY_HOURS)
ca_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "MCG disposable updater UAT CA")])
server_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "MCG disposable updater UAT server")])
ca_key = rsa.generate_private_key(public_exponent=65537, key_size=3072)
ca_certificate = (
    x509.CertificateBuilder()
    .subject_name(ca_name)
    .issuer_name(ca_name)
    .public_key(ca_key.public_key())
    .serial_number(x509.random_serial_number())
    .not_valid_before(not_before)
    .not_valid_after(not_after)
    .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
    .add_extension(
        x509.KeyUsage(
            digital_signature=True,
            content_commitment=False,
            key_encipherment=False,
            data_encipherment=False,
            key_agreement=False,
            key_cert_sign=True,
            crl_sign=True,
            encipher_only=None,
            decipher_only=None,
        ),
        critical=True,
    )
    .add_extension(x509.SubjectKeyIdentifier.from_public_key(ca_key.public_key()), critical=False)
    .sign(ca_key, hashes.SHA256())
)
server_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
server_certificate = (
    x509.CertificateBuilder()
    .subject_name(server_name)
    .issuer_name(ca_name)
    .public_key(server_key.public_key())
    .serial_number(x509.random_serial_number())
    .not_valid_before(not_before)
    .not_valid_after(not_after)
    .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
    .add_extension(
        x509.KeyUsage(
            digital_signature=True,
            content_commitment=False,
            key_encipherment=True,
            data_encipherment=False,
            key_agreement=False,
            key_cert_sign=False,
            crl_sign=False,
            encipher_only=None,
            decipher_only=None,
        ),
        critical=True,
    )
    .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), critical=False)
    .add_extension(
        x509.SubjectAlternativeName(
            [x509.DNSName("localhost"), x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]
        ),
        critical=False,
    )
    .add_extension(x509.SubjectKeyIdentifier.from_public_key(server_key.public_key()), critical=False)
    .add_extension(x509.AuthorityKeyIdentifier.from_issuer_public_key(ca_key.public_key()), critical=False)
    .sign(ca_key, hashes.SHA256())
)

ca_pem = ca_certificate.public_bytes(serialization.Encoding.PEM)
server_pem = server_certificate.public_bytes(serialization.Encoding.PEM)
server_key_pem = server_key.private_bytes(
    serialization.Encoding.PEM,
    serialization.PrivateFormat.PKCS8,
    serialization.NoEncryption(),
)
created: list[pathlib.Path] = []
try:
    write_new(ca_path, ca_pem, 0o600)
    created.append(ca_path)
    write_new(server_path, server_pem + ca_pem, 0o600)
    created.append(server_path)
    write_new(key_path, server_key_pem, 0o600)
    created.append(key_path)
except Exception:
    for candidate in created:
        candidate.unlink(missing_ok=True)
    raise

print(
    json.dumps(
        {
            "schema_version": 1,
            "validity_hours": VALIDITY_HOURS,
            "ca_sha256": hashlib.sha256(ca_certificate.public_bytes(serialization.Encoding.DER)).hexdigest(),
            "server_sha256": hashlib.sha256(
                server_certificate.public_bytes(serialization.Encoding.DER)
            ).hexdigest(),
            "not_after": not_after.isoformat().replace("+00:00", "Z"),
            "hosts": ["localhost", "127.0.0.1"],
        },
        separators=(",", ":"),
    )
)
