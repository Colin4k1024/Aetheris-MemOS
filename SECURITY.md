# Security Policy

## Supported Versions

Security updates are provided for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you believe you have found a security vulnerability in the Adaptive Memory Management System, please report it responsibly:

1. **Contact**: Open a [GitHub Security Advisory](https://github.com/YOUR_ORG/adaptive-memory-system/security/advisories/new) (recommended) or email the maintainers if the repository is under an organization with a disclosed security contact.

2. **Include**:
   - Type of issue (e.g. authentication bypass, injection, information disclosure)
   - Full path and affected source code location
   - Step-by-step instructions to reproduce
   - Potential impact and suggested fix (if any)

3. **Response**: We aim to acknowledge reports within 48 hours and will keep you updated on progress.

4. **Disclosure**: We follow coordinated disclosure. We will not disclose the issue publicly before a fix is available, and we will credit reporters in the advisory unless they prefer to remain anonymous.

## Security Expectations

- **Dependencies**: Keep backend (`cargo update`) and frontend (`npm audit`) dependencies up to date.
- **Secrets**: Do not commit API keys, JWT secrets, or database credentials. Use environment variables or secret managers.
- **Authentication**: Use strong JWT secrets in production; change default credentials before deployment.

Thank you for helping keep this project secure.
