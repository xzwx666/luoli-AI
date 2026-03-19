"""
安全策略模块

提供命令黑白名单、权限管控等功能。
"""

import re
import logging
from typing import List, Set, Optional
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class SecurityPolicy:
    """
    安全策略管理器
    
    管理命令的黑白名单，控制命令执行权限。
    
    Attributes:
        whitelist: 命令白名单（正则表达式列表）
        blacklist: 命令黑名单（正则表达式列表）
        forbidden_patterns: 禁止的模式（如密码、密钥等敏感词）
    """
    
    whitelist: List[str] = field(default_factory=list)
    blacklist: List[str] = field(default_factory=list)
    forbidden_patterns: List[str] = field(default_factory=lambda: [
        "password", "secret", "token", "api_key", "private_key"
    ])
    max_command_length: int = 4096
    
    def __post_init__(self):
        """初始化后编译正则表达式"""
        self._whitelist_patterns: List[re.Pattern] = []
        self._blacklist_patterns: List[re.Pattern] = []
        self._compile_patterns()
    
    def _compile_patterns(self):
        """编译正则表达式模式"""
        for pattern in self.whitelist:
            try:
                self._whitelist_patterns.append(re.compile(pattern))
            except re.error as e:
                logger.warning(f"无效的白名单正则表达式 '{pattern}': {e}")
        
        for pattern in self.blacklist:
            try:
                self._blacklist_patterns.append(re.compile(pattern))
            except re.error as e:
                logger.warning(f"无效的黑名单正则表达式 '{pattern}': {e}")
    
    def check_command(self, command: str, mode: str = "normal") -> bool:
        """
        检查命令是否允许执行
        
        Args:
            command: 要检查的命令
            mode: 运行模式，'normal' 或 'strict'
            
        Returns:
            True 如果命令允许执行，False 否则
        """
        # 检查命令长度
        if len(command) > self.max_command_length:
            logger.warning(f"命令长度超过限制: {len(command)} > {self.max_command_length}")
            return False
        
        # 检查禁止的模式
        cmd_lower = command.lower()
        for pattern in self.forbidden_patterns:
            if pattern in cmd_lower:
                logger.warning(f"命令包含禁止的模式: {pattern}")
                return False
        
        # 检查黑名单
        for pattern in self._blacklist_patterns:
            if pattern.search(command):
                logger.warning(f"命令匹配黑名单: {pattern.pattern}")
                return False
        
        # 严格模式下检查白名单
        if mode == "strict":
            if not self._whitelist_patterns:
                logger.warning("严格模式下白名单为空，拒绝所有命令")
                return False
            
            allowed = any(p.search(command) for p in self._whitelist_patterns)
            if not allowed:
                logger.warning(f"严格模式：命令不在白名单中: {command}")
                return False
        
        return True
    
    def add_whitelist(self, pattern: str) -> bool:
        """
        添加白名单规则
        
        Args:
            pattern: 正则表达式模式
            
        Returns:
            True 如果添加成功
        """
        try:
            compiled = re.compile(pattern)
            self.whitelist.append(pattern)
            self._whitelist_patterns.append(compiled)
            logger.info(f"已添加白名单: {pattern}")
            return True
        except re.error as e:
            logger.error(f"添加白名单失败 '{pattern}': {e}")
            return False
    
    def add_blacklist(self, pattern: str) -> bool:
        """
        添加黑名单规则
        
        Args:
            pattern: 正则表达式模式
            
        Returns:
            True 如果添加成功
        """
        try:
            compiled = re.compile(pattern)
            self.blacklist.append(pattern)
            self._blacklist_patterns.append(compiled)
            logger.info(f"已添加黑名单: {pattern}")
            return True
        except re.error as e:
            logger.error(f"添加黑名单失败 '{pattern}': {e}")
            return False
    
    def remove_whitelist(self, pattern: str) -> bool:
        """移除白名单规则"""
        if pattern in self.whitelist:
            self.whitelist.remove(pattern)
            self._compile_patterns()
            logger.info(f"已移除白名单: {pattern}")
            return True
        return False
    
    def remove_blacklist(self, pattern: str) -> bool:
        """移除黑名单规则"""
        if pattern in self.blacklist:
            self.blacklist.remove(pattern)
            self._compile_patterns()
            logger.info(f"已移除黑名单: {pattern}")
            return True
        return False
    
    def list_rules(self) -> dict:
        """
        列出所有规则
        
        Returns:
            包含白名单和黑名单的字典
        """
        return {
            "whitelist": self.whitelist.copy(),
            "blacklist": self.blacklist.copy(),
            "forbidden_patterns": self.forbidden_patterns.copy(),
        }
    
    def to_dict(self) -> dict:
        """转换为字典"""
        return {
            "whitelist": self.whitelist,
            "blacklist": self.blacklist,
            "forbidden_patterns": self.forbidden_patterns,
            "max_command_length": self.max_command_length,
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> "SecurityPolicy":
        """从字典创建实例"""
        return cls(
            whitelist=data.get("whitelist", []),
            blacklist=data.get("blacklist", []),
            forbidden_patterns=data.get("forbidden_patterns", [
                "password", "secret", "token", "api_key", "private_key"
            ]),
            max_command_length=data.get("max_command_length", 4096),
        )


class ToolPolicy:
    """
    工具使用策略
    
    管理 MCP 工具的黑白名单。
    """
    
    def __init__(self):
        self.whitelist: Set[str] = set()
        self.blacklist: Set[str] = set()
    
    def check_tool(self, tool_name: str, mode: str = "normal") -> bool:
        """
        检查工具是否允许使用
        
        Args:
            tool_name: 工具名称
            mode: 运行模式
            
        Returns:
            True 如果允许使用
        """
        # 检查黑名单
        if tool_name in self.blacklist:
            logger.warning(f"工具在黑名单中: {tool_name}")
            return False
        
        # 严格模式下检查白名单
        if mode == "strict":
            if tool_name not in self.whitelist:
                logger.warning(f"严格模式：工具不在白名单中: {tool_name}")
                return False
        
        return True
    
    def add_whitelist(self, tool_name: str):
        """添加工具白名单"""
        self.whitelist.add(tool_name)
        logger.info(f"已添加工具白名单: {tool_name}")
    
    def add_blacklist(self, tool_name: str):
        """添加工具黑名单"""
        self.blacklist.add(tool_name)
        logger.info(f"已添加工具黑名单: {tool_name}")
    
    def remove_whitelist(self, tool_name: str):
        """移除工具白名单"""
        self.whitelist.discard(tool_name)
    
    def remove_blacklist(self, tool_name: str):
        """移除工具黑名单"""
        self.blacklist.discard(tool_name)


class ApiPolicy:
    """
    API 访问策略
    
    管理外部 API 的黑白名单。
    """
    
    def __init__(self):
        self.whitelist: Set[str] = set()
        self.blacklist: Set[str] = set()
    
    def check_api(self, url: str, mode: str = "normal") -> bool:
        """
        检查 API 是否允许访问
        
        Args:
            url: API URL
            mode: 运行模式
            
        Returns:
            True 如果允许访问
        """
        # 检查黑名单
        for blocked in self.blacklist:
            if blocked in url:
                logger.warning(f"API 在黑名单中: {url} (匹配: {blocked})")
                return False
        
        # 严格模式下检查白名单
        if mode == "strict":
            allowed = any(allowed in url for allowed in self.whitelist)
            if not allowed:
                logger.warning(f"严格模式：API 不在白名单中: {url}")
                return False
        
        return True
    
    def add_whitelist(self, url_pattern: str):
        """添加 API 白名单"""
        self.whitelist.add(url_pattern)
        logger.info(f"已添加 API 白名单: {url_pattern}")
    
    def add_blacklist(self, url_pattern: str):
        """添加 API 黑名单"""
        self.blacklist.add(url_pattern)
        logger.info(f"已添加 API 黑名单: {url_pattern}")
