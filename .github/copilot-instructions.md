# GitHub Copilot Instructions

## Code Style and Documentation

### Emoji Usage

- Do not use emojis in log outputs
- Do not use emojis in README files
- Keep all outputs and documentation emoji-free

### README Files

- Do NOT create README files automatically
- README files are only created upon explicit user instruction
- Wait for express confirmation before generating a README

### Project Structure

#### Folder Organization

- Create a sensible, modular folder structure
- Avoid placing all files in a single folder (e.g. `src/`)
- Divide code by logical areas and functionality

#### Structuring Principles

- Group related files in their own subfolders
- Each logical strand/area gets its own folder
- Create hierarchies that reflect the architecture
- Example structure:

src/
├── components/
│ ├── ui/
│ └── forms/
├── services/
│ ├── api/
│ └── auth/
├── utils/
└── models/

#### Goal

- Clarity and maintainability through clear separation
- Avoidance of overcrowded folders
- Intuitive navigation through the project

## Security Requirements

### Zero Trust Architecture

- Assume Zero Trust architecture principles in all implementations
- Never trust, always verify - apply verification at every layer

### Endpoint Security

- Do NOT create open or unsecured endpoints
- All endpoints must require authentication and authorization
- Implement proper access controls for every API route

### Security Best Practices

- No hardcoded credentials or secrets in code
- Validate and sanitize all user inputs
- Implement proper error handling without exposing sensitive information
- Use secure communication protocols (HTTPS/TLS)
- Apply principle of least privilege
- Implement rate limiting and request throttling
- Use secure session management
- Enable CORS policies appropriately - never use wildcard (\*) in production

### Implementation Safety

- Avoid any implementation that could compromise security
- Flag potential security vulnerabilities during development
- Prioritize security over convenience
- Conduct security checks before exposing any functionality
