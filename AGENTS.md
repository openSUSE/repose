# AGENTS.md
# Guidelines for Agentic Coding in the Repose Repository

## Table of Contents
- [Build/Lint/Test Commands](#buildlinttest-commands)
- [Code Style Guidelines](#code-style-guidelines)
  - [Imports](#imports)
  - [Formatting](#formatting)
  - [Type Hints](#type-hints)
  - [Naming Conventions](#naming-conventions)
  - [Error Handling](#error-handling)
  - [Documentation](#documentation)
- [Testing](#testing)
- [Git Workflow](#git-workflow)

## Build/Lint/Test Commands

### Installing Dependencies
```bash
uv pip install -r requirements.txt          # Main dependencies (paramiko, ruamel.yaml)
uv pip install -r requirements-test.txt     # Test dependencies (pytest, pytest-cov)
```

### Running Tests
- **All tests**: `python3 -m pytest -v --cov=repose`
- **Single test file**: `python3 -m pytest tests/test_add_command.py -v`
- **Single test function**: `python3 -m pytest tests/test_add_command.py::test_add_command_run -v`
- **With coverage**: `python3 -m pytest --cov=repose --cov-report=term-missing`

### Linting and Formatting
- **Check formatting**: `ruff format --diff .`
- **Apply formatting**: `ruff format .`
- **Lint code**: `ruff check .`

### Code Quality
The project uses Ruff for linting and formatting. All code should pass:
- `ruff format --diff .` (no differences)
- `ruff check .` (no linting errors)

## Code Style Guidelines

### Imports
- **Absolute imports**: Always use absolute imports (e.g., `from repose.command.add import Add`)
- **Standard library first**: Group imports in this order:
  1. Standard library
  2. Third-party libraries
  3. Local application imports
- **No wildcard imports**: Never use `from module import *`
- **Sort alphabetically**: Within each group, sort imports alphabetically

Example:
```python
import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock, call

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.add import Add
from repose.types.repa import Repa
```

### Formatting
- **Line length**: 79 characters (PEP 8 standard)
- **Indentation**: 4 spaces (no tabs)
- **Blank lines**: 
  - 2 blank lines around top-level functions/classes
  - 1 blank line within functions/classes between logical sections
- **Spaces**: Follow PEP 8 spacing rules (e.g., `function(arg1, arg2)` not `function(arg1,arg2)`)

### Type Hints
- **Required**: All functions and methods should have type hints
- **Variable annotations**: Use when it improves clarity (especially for complex types)
- **Return types**: Always specify return types
- **Exception**: Simple internal functions may omit hints if the context is clear

Example:
```python
def solve_repa(self, repa: Repa, base: str) -> dict:
    """Solve repository pattern to actual repositories."""
    # Implementation
```

### Naming Conventions
- **Snake_case**: For variables, functions, and methods (e.g., `solve_repa`)
- **CamelCase**: For class names (e.g., `class AddCommand`)
- **UPPER_CASE**: For constants and environment variables
- **Descriptive names**: Avoid abbreviations unless widely understood (e.g., `repo` not `rep`)
- **Private members**: Prefix with underscore (e.g., `_init_repoq`)

### Error Handling
- **Specific exceptions**: Catch specific exceptions, not bare `Exception`
- **Context**: Include relevant context in error messages
- **Logging**: Use the logger for debug/info/warning messages
- **Propagate**: Let exceptions bubble up when appropriate (don't swallow them)

Example:
```python
try:
    result = self._execute_command(cmd)
except paramiko.ssh_exception.SSHException as e:
    logger.error(f"SSH command failed: {e}")
    raise
```

### Documentation
- **Docstrings**: All public functions/classes should have docstrings
- **Format**: Follow Google style or reST format
- **Examples**: Include usage examples when helpful
- **Type information**: Document parameters and return values

Example:
```python
def solve_repa(self, repa: Repa, base: str) -> dict:
    """Solve repository pattern to actual repositories.

    Args:
        repa: Repository pattern to solve
        base: Base product identifier

    Returns:
        Dictionary mapping products to repository lists

    Raises:
        ValueError: If the repository pattern is invalid
    """
```

## Testing
- **Test location**: All tests in `tests/` directory
- **Framework**: pytest (with pytest-cov for coverage)
- **Fixtures**: Use conftest.py for shared fixtures
- **Mocking**: Use unittest.mock.MagicMock for external dependencies
- **Test names**: Descriptive names (e.g., `test_add_command_run`)

### Test Structure
```python
def test_function_name(monkeypatch, fixture1, fixture2):
    # Setup
    setup_data = prepare_test_data()

    # Exercise
    result = function_under_test(setup_data)

    # Verify
    assert result == expected_value
```

### Common Patterns
- **Mock SSH connections**: Use `mock_ssh_client` fixture from conftest.py
- **Immediate executor**: Use `ImmediateExecutor` for concurrent operations
- **Assertions**: Use pytest assertions (no unittest.TestCase)

## Git Workflow
- **Branch naming**: Use descriptive names (e.g., `feature/add-sdk-support`)
- **Commit messages**: Follow conventional commits format
  - `feat: add new command`
  - `fix: handle SSH timeout`
  - `docs: update README`
- **Rebasing**: Rebase feature branches before merging
- **Testing**: Run tests before committing (`python3 -m pytest`)
- **Linting**: Run linters before committing (`ruff check .`)

## Continuous Integration
The project uses GitHub Actions for CI:
- **Linting**: Runs on all pushes/pull requests
  - Checks formatting with `ruff format`
  - Lints code with `ruff check`
- **Testing**: Runs on all pushes/pull requests
  - Tests with Python 3.11 and 3.13
  - Reports coverage statistics
- **CodeQL**: Runs on master branch and pull requests
  - Performs static analysis for security vulnerabilities

## Architecture Overview
- **Commands**: In `repose/command/` (add, remove, install, etc.)
- **Types**: In `repose/types/` (data structures)
- **Target**: In `repose/target/` (host operations)
- **Main entry point**: `repose/main.py`

## Key Dependencies
- **paramiko**: SSH client library
- **ruamel.yaml**: YAML parsing (supports round-trip)
- **pytest**: Testing framework
