"""
Obscura Next Python SDK — Data models.
"""

from dataclasses import dataclass, field
from typing import Optional, Dict, Any, List


@dataclass
class SessionConfig:
    """Configuration for a browser session."""
    proxy_url: Optional[str] = None
    stealth: bool = True
    user_agent: Optional[str] = None
    allow_file_access: bool = False
    allow_private_network: bool = False
    timeout_ms: int = 30000


@dataclass
class AgentAction:
    """Describes a high-level agent action."""
    action: str
    url: Optional[str] = None
    selector: Optional[str] = None
    text: Optional[str] = None
    clear: bool = False
    format: Optional[str] = None
    wait_until: str = "load"


@dataclass
class ActionResult:
    """Result of a session operation (navigate, evaluate, click, fill, type)."""
    action: str
    success: bool
    payload: Optional[Any] = None
    elapsed_ms: float = 0.0
    error: Optional[Any] = None


@dataclass
class SemanticNode:
    """A node in the accessibility / semantic tree snapshot."""
    id: str
    tag: str
    text: str
    selector: str
    role: Optional[str] = None
    is_interactive: bool = False
    attributes: Dict[str, str] = field(default_factory=dict)
    children: List["SemanticNode"] = field(default_factory=list)
