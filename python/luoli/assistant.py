"""
洛璃终端助手主类
"""

import subprocess
import json
import logging
from typing import Optional, List, Dict, Any, Tuple
from pathlib import Path
from datetime import datetime

from .security import SecurityPolicy
from .monitor import ActivityMonitor

logger = logging.getLogger(__name__)


class Assistant:
    """
    洛璃个人终端助手
    
    提供安全的命令执行环境，支持权限管控和操作监控。
    
    Attributes:
        mode: 运行模式，'normal' 或 'strict'
        security: 安全策略管理器
        monitor: 活动监控器
    """
    
    def __init__(self, mode: str = "normal", config_path: Optional[str] = None):
        """
        初始化助手
        
        Args:
            mode: 运行模式，'normal'(普通模式) 或 'strict'(严格模式)
            config_path: 配置文件路径
        """
        self.mode = mode
        self.config_path = config_path or str(Path.home() / ".luoli" / "config.json")
        self.security = SecurityPolicy()
        self.monitor = ActivityMonitor()
        
        # 加载配置
        self._load_config()
        
        logger.info(f"洛璃助手初始化完成，当前模式: {mode}")
    
    def _load_config(self):
        """加载配置文件"""
        config_file = Path(self.config_path)
        if config_file.exists():
            try:
                with open(config_file, 'r', encoding='utf-8') as f:
                    config = json.load(f)
                    self.mode = config.get('mode', self.mode)
                    logger.info(f"已加载配置文件: {self.config_path}")
            except Exception as e:
                logger.warning(f"加载配置文件失败: {e}")
    
    def execute(self, command: str, timeout: Optional[int] = None) -> Dict[str, Any]:
        """
        执行命令
        
        Args:
            command: 要执行的命令
            timeout: 超时时间（秒）
            
        Returns:
            包含执行结果的字典
            
        Raises:
            SecurityError: 当命令违反安全策略时
            ExecutionError: 当命令执行失败时
        """
        # 安全检查
        if not self.security.check_command(command, self.mode):
            self.monitor.log_blocked(command, "security_policy")
            raise SecurityError(f"命令违反安全策略: {command}")
        
        # 记录开始执行
        start_time = datetime.now()
        self.monitor.log_start(command)
        
        try:
            # 执行命令
            result = subprocess.run(
                command,
                shell=True,
                capture_output=True,
                text=True,
                timeout=timeout
            )
            
            # 记录执行结果
            end_time = datetime.now()
            duration = (end_time - start_time).total_seconds()
            
            success = result.returncode == 0
            self.monitor.log_complete(
                command=command,
                success=success,
                duration=duration,
                output=result.stdout if success else result.stderr
            )
            
            return {
                'success': success,
                'returncode': result.returncode,
                'stdout': result.stdout,
                'stderr': result.stderr,
                'duration': duration,
            }
            
        except subprocess.TimeoutExpired:
            self.monitor.log_complete(
                command=command,
                success=False,
                duration=timeout or 0,
                output="执行超时"
            )
            raise ExecutionError(f"命令执行超时: {command}")
            
        except Exception as e:
            self.monitor.log_complete(
                command=command,
                success=False,
                duration=0,
                output=str(e)
            )
            raise ExecutionError(f"命令执行失败: {e}")
    
    def execute_safe(self, command: str, allowed_commands: List[str]) -> Dict[str, Any]:
        """
        在安全列表中执行命令
        
        Args:
            command: 要执行的命令
            allowed_commands: 允许的命令列表
            
        Returns:
            执行结果
        """
        cmd_base = command.split()[0] if command.split() else ""
        
        if cmd_base not in allowed_commands:
            raise SecurityError(f"命令 '{cmd_base}' 不在允许列表中")
        
        return self.execute(command)
    
    def set_mode(self, mode: str):
        """
        设置运行模式
        
        Args:
            mode: 'normal' 或 'strict'
        """
        if mode not in ('normal', 'strict'):
            raise ValueError("模式必须是 'normal' 或 'strict'")
        
        self.mode = mode
        self.monitor.log_mode_change(mode)
        logger.info(f"运行模式已切换为: {mode}")
    
    def get_mode(self) -> str:
        """获取当前运行模式"""
        return self.mode
    
    def add_to_whitelist(self, pattern: str):
        """
        添加命令到白名单
        
        Args:
            pattern: 命令模式（支持正则表达式）
        """
        self.security.add_whitelist(pattern)
        logger.info(f"已添加白名单: {pattern}")
    
    def add_to_blacklist(self, pattern: str):
        """
        添加命令到黑名单
        
        Args:
            pattern: 命令模式（支持正则表达式）
        """
        self.security.add_blacklist(pattern)
        logger.info(f"已添加黑名单: {pattern}")
    
    def get_logs(self, limit: int = 50) -> List[Dict[str, Any]]:
        """
        获取最近的日志记录
        
        Args:
            limit: 返回的记录数量
            
        Returns:
            日志记录列表
        """
        return self.monitor.get_recent_logs(limit)
    
    def get_statistics(self) -> Dict[str, Any]:
        """获取执行统计信息"""
        return self.monitor.get_statistics()
    
    def interactive_shell(self):
        """启动交互式终端"""
        print(f"🦞 洛璃终端助手 - 交互式模式")
        print(f"当前模式: {self.mode}")
        print("输入 'help' 查看帮助, 'exit' 退出\n")
        
        while True:
            try:
                command = input("luoli> ").strip()
                
                if not command:
                    continue
                
                if command in ('exit', 'quit'):
                    print("再见!")
                    break
                
                if command == 'help':
                    self._print_help()
                    continue
                
                if command == 'mode':
                    print(f"当前模式: {self.mode}")
                    continue
                
                # 执行命令
                result = self.execute(command)
                
                if result['success']:
                    if result['stdout']:
                        print(result['stdout'])
                else:
                    print(f"错误: {result['stderr']}", file=__import__('sys').stderr)
                    
            except SecurityError as e:
                print(f"安全错误: {e}", file=__import__('sys').stderr)
            except ExecutionError as e:
                print(f"执行错误: {e}", file=__import__('sys').stderr)
            except KeyboardInterrupt:
                print("\n使用 'exit' 退出")
            except EOFError:
                print("\n再见!")
                break
    
    def _print_help(self):
        """打印帮助信息"""
        print("\n可用命令:")
        print("  help       - 显示帮助")
        print("  mode       - 查看当前模式")
        print("  exit/quit  - 退出终端")
        print("\n其他任意命令将被执行（根据当前模式进行权限检查）\n")


class SecurityError(Exception):
    """安全策略违规错误"""
    pass


class ExecutionError(Exception):
    """命令执行错误"""
    pass
