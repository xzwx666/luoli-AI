"""
操作行为监控模块

提供命令执行监控、日志记录、审计功能。
"""

import json
import logging
from typing import List, Dict, Any, Optional
from dataclasses import dataclass, field, asdict
from datetime import datetime
from pathlib import Path
from enum import Enum
import threading

logger = logging.getLogger(__name__)


class ActivityType(Enum):
    """活动类型"""
    COMMAND_EXECUTION = "command_execution"
    TOOL_INVOCATION = "tool_invocation"
    API_CALL = "api_call"
    FILE_ACCESS = "file_access"
    MODE_CHANGE = "mode_change"
    CONFIG_CHANGE = "config_change"
    SECURITY_VIOLATION = "security_violation"


class ActivityStatus(Enum):
    """活动状态"""
    SUCCESS = "success"
    FAILED = "failed"
    BLOCKED = "blocked"
    PENDING = "pending"


@dataclass
class ActivityRecord:
    """
    活动记录
    
    记录一次命令执行或操作的详细信息。
    """
    id: str
    timestamp: datetime
    activity_type: ActivityType
    user: str
    command: str
    args: List[str] = field(default_factory=list)
    working_dir: str = ""
    status: ActivityStatus = ActivityStatus.PENDING
    duration: float = 0.0
    output: str = ""
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return {
            "id": self.id,
            "timestamp": self.timestamp.isoformat(),
            "activity_type": self.activity_type.value,
            "user": self.user,
            "command": self.command,
            "args": self.args,
            "working_dir": self.working_dir,
            "status": self.status.value,
            "duration": self.duration,
            "output": self.output,
            "metadata": self.metadata,
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ActivityRecord":
        """从字典创建"""
        return cls(
            id=data["id"],
            timestamp=datetime.fromisoformat(data["timestamp"]),
            activity_type=ActivityType(data["activity_type"]),
            user=data["user"],
            command=data["command"],
            args=data.get("args", []),
            working_dir=data.get("working_dir", ""),
            status=ActivityStatus(data["status"]),
            duration=data.get("duration", 0.0),
            output=data.get("output", ""),
            metadata=data.get("metadata", {}),
        )


class ActivityMonitor:
    """
    活动监控器
    
    监控和记录所有操作行为，提供审计功能。
    
    Attributes:
        log_file: 日志文件路径
        max_records: 内存中保留的最大记录数
    """
    
    def __init__(self, log_file: Optional[str] = None, max_records: int = 1000):
        """
        初始化监控器
        
        Args:
            log_file: 日志文件路径
            max_records: 内存中保留的最大记录数
        """
        self.log_file = Path(log_file) if log_file else Path.home() / ".luoli" / "logs" / "activity.log"
        self.max_records = max_records
        self.records: List[ActivityRecord] = []
        self.lock = threading.Lock()
        
        # 确保日志目录存在
        self.log_file.parent.mkdir(parents=True, exist_ok=True)
        
        logger.info(f"活动监控器初始化完成，日志文件: {self.log_file}")
    
    def _generate_id(self) -> str:
        """生成唯一ID"""
        import uuid
        return str(uuid.uuid4())[:8]
    
    def log_start(self, command: str, activity_type: ActivityType = ActivityType.COMMAND_EXECUTION):
        """
        记录操作开始
        
        Args:
            command: 命令
            activity_type: 活动类型
        """
        import getpass
        import os
        
        record = ActivityRecord(
            id=self._generate_id(),
            timestamp=datetime.now(),
            activity_type=activity_type,
            user=getpass.getuser(),
            command=command,
            working_dir=os.getcwd(),
            status=ActivityStatus.PENDING,
        )
        
        with self.lock:
            self.records.append(record)
            
            # 限制记录数量
            if len(self.records) > self.max_records:
                self.records.pop(0)
        
        return record.id
    
    def log_complete(
        self,
        command: str,
        success: bool,
        duration: float,
        output: str = "",
        record_id: Optional[str] = None,
    ):
        """
        记录操作完成
        
        Args:
            command: 命令
            success: 是否成功
            duration: 执行时长（秒）
            output: 输出内容
            record_id: 记录ID（如果为None则查找最近的匹配记录）
        """
        with self.lock:
            # 查找记录
            if record_id:
                record = next((r for r in self.records if r.id == record_id), None)
            else:
                # 查找最近的匹配命令的待处理记录
                record = next(
                    (r for r in reversed(self.records) 
                     if r.command == command and r.status == ActivityStatus.PENDING),
                    None
                )
            
            if record:
                record.status = ActivityStatus.SUCCESS if success else ActivityStatus.FAILED
                record.duration = duration
                record.output = output[:1000]  # 限制输出长度
                
                # 写入日志文件
                self._write_to_log(record)
    
    def log_blocked(self, command: str, reason: str):
        """
        记录被阻止的操作
        
        Args:
            command: 命令
            reason: 阻止原因
        """
        import getpass
        import os
        
        record = ActivityRecord(
            id=self._generate_id(),
            timestamp=datetime.now(),
            activity_type=ActivityType.SECURITY_VIOLATION,
            user=getpass.getuser(),
            command=command,
            working_dir=os.getcwd(),
            status=ActivityStatus.BLOCKED,
            metadata={"reason": reason},
        )
        
        with self.lock:
            self.records.append(record)
            self._write_to_log(record)
    
    def log_mode_change(self, new_mode: str):
        """
        记录模式切换
        
        Args:
            new_mode: 新模式
        """
        import getpass
        import os
        
        record = ActivityRecord(
            id=self._generate_id(),
            timestamp=datetime.now(),
            activity_type=ActivityType.MODE_CHANGE,
            user=getpass.getuser(),
            command=f"mode -> {new_mode}",
            working_dir=os.getcwd(),
            status=ActivityStatus.SUCCESS,
            metadata={"new_mode": new_mode},
        )
        
        with self.lock:
            self.records.append(record)
            self._write_to_log(record)
    
    def _write_to_log(self, record: ActivityRecord):
        """写入日志文件"""
        try:
            with open(self.log_file, "a", encoding="utf-8") as f:
                f.write(json.dumps(record.to_dict(), ensure_ascii=False) + "\n")
        except Exception as e:
            logger.error(f"写入日志失败: {e}")
    
    def get_recent_logs(self, limit: int = 50) -> List[Dict[str, Any]]:
        """
        获取最近的日志记录
        
        Args:
            limit: 返回的记录数量
            
        Returns:
            日志记录列表
        """
        with self.lock:
            return [r.to_dict() for r in self.records[-limit:]]
    
    def get_statistics(self) -> Dict[str, Any]:
        """
        获取执行统计信息
        
        Returns:
            统计信息字典
        """
        with self.lock:
            total = len(self.records)
            success = sum(1 for r in self.records if r.status == ActivityStatus.SUCCESS)
            failed = sum(1 for r in self.records if r.status == ActivityStatus.FAILED)
            blocked = sum(1 for r in self.records if r.status == ActivityStatus.BLOCKED)
            
            by_type = {}
            for r in self.records:
                type_name = r.activity_type.value
                by_type[type_name] = by_type.get(type_name, 0) + 1
            
            return {
                "total": total,
                "success": success,
                "failed": failed,
                "blocked": blocked,
                "by_type": by_type,
            }
    
    def search_logs(
        self,
        keyword: Optional[str] = None,
        activity_type: Optional[ActivityType] = None,
        status: Optional[ActivityStatus] = None,
        start_time: Optional[datetime] = None,
        end_time: Optional[datetime] = None,
    ) -> List[Dict[str, Any]]:
        """
        搜索日志记录
        
        Args:
            keyword: 关键词
            activity_type: 活动类型
            status: 状态
            start_time: 开始时间
            end_time: 结束时间
            
        Returns:
            匹配的日志记录列表
        """
        results = []
        
        with self.lock:
            for record in self.records:
                # 关键词过滤
                if keyword and keyword not in record.command:
                    continue
                
                # 类型过滤
                if activity_type and record.activity_type != activity_type:
                    continue
                
                # 状态过滤
                if status and record.status != status:
                    continue
                
                # 时间过滤
                if start_time and record.timestamp < start_time:
                    continue
                if end_time and record.timestamp > end_time:
                    continue
                
                results.append(record.to_dict())
        
        return results
    
    def export_logs(self, output_path: str, format: str = "json"):
        """
        导出日志
        
        Args:
            output_path: 输出文件路径
            format: 格式，'json' 或 'csv'
        """
        records = self.get_recent_logs(len(self.records))
        
        if format == "json":
            with open(output_path, "w", encoding="utf-8") as f:
                json.dump(records, f, ensure_ascii=False, indent=2)
        elif format == "csv":
            import csv
            if records:
                with open(output_path, "w", newline="", encoding="utf-8") as f:
                    writer = csv.DictWriter(f, fieldnames=records[0].keys())
                    writer.writeheader()
                    writer.writerows(records)
        else:
            raise ValueError(f"不支持的格式: {format}")
        
        logger.info(f"日志已导出到: {output_path}")
    
    def clear_logs(self):
        """清空日志"""
        with self.lock:
            self.records.clear()
        
        if self.log_file.exists():
            self.log_file.unlink()
        
        logger.info("日志已清空")
