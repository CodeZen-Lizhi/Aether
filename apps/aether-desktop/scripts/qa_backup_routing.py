"""用真实本地网关验证调度优先级删除、历史升级、完整备份及恢复。

用法：python3 qa_backup_routing.py NEW_GATEWAY [--legacy OLD_GATEWAY]
只创建隔离数据库和合成凭据，不读写用户数据库。
"""
import argparse
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import uuid

from qa_api import Session


class Gateway:
    """管理一个隔离数据库和仅监听回环地址的网关，允许换程序重启验证升级。"""

    def __init__(self):
        """创建私有目录和仅在内存中保存的测试秘密。"""
        self.root = Path(tempfile.mkdtemp(prefix="aether-backup-routing-"))
        self.root.chmod(0o700)
        self.child = None
        self.env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR", "LANG") if key in os.environ}
        self.env.update(ENVIRONMENT="production", AETHER_DATABASE_DRIVER="sqlite",
                        AETHER_DATABASE_URL="sqlite://" + str(self.root / "test.db"),
                        AETHER_RUNTIME_BACKEND="memory", AETHER_GATEWAY_AUTO_PREPARE_DATABASE="true",
                        JWT_SECRET_KEY=uuid.uuid4().hex * 2, ENCRYPTION_KEY=uuid.uuid4().hex * 2,
                        ADMIN_USERNAME="desktopqa", ADMIN_PASSWORD="Aether-QA-2026!",
                        AUTH_REFRESH_COOKIE_SECURE="false")

    def start(self, binary):
        """启动指定真实程序并等待认证成功；启动错误保存在私有日志。"""
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            port = reserved.getsockname()[1]
        with (self.root / "gateway.log").open("ab") as log:
            self.child = subprocess.Popen([
                str(Path(binary).resolve()), "--app-host", "127.0.0.1", "--app-port", str(port),
                "--listener-shards", "1", "--shutdown-timeout-seconds", "2", "--exit-on-stdin-close",
            ], stdin=subprocess.PIPE, stdout=log, stderr=log, env=self.env, cwd=self.root)
        self.session = Session({"base": f"http://127.0.0.1:{port}"})
        for _ in range(200):
            if self.child.poll() is not None:
                raise AssertionError("网关提前退出，查看私有日志")
            try:
                self.session.login()
                return
            except (urllib.error.URLError, TimeoutError):
                time.sleep(0.1)
        raise AssertionError("本地网关未就绪")

    def stop(self):
        """只停止本测试持有的网关子进程。"""
        if self.child and self.child.poll() is None:
            self.child.stdin.close()
            try:
                self.child.wait(timeout=8)
            except subprocess.TimeoutExpired:
                self.child.kill()
                self.child.wait()


def provider(session, name):
    """通过真实管理接口创建供应商、端点和可解密的渠道密钥。"""
    entity = session.json("POST", "/api/admin/providers/", {
        "name": name, "provider_type": "custom", "billing_type": "pay_as_you_go", "is_active": True,
    })
    pid = entity["id"]
    session.json("POST", f"/api/admin/endpoints/providers/{pid}/endpoints", {
        "provider_id": pid, "api_format": "openai:chat", "base_url": "http://127.0.0.1:1", "is_active": True,
    })
    key = session.json("POST", f"/api/admin/endpoints/providers/{pid}/keys", {
        "name": name + " key", "api_formats": ["openai:chat"], "api_key": "sk-synthetic-local-only",
        "is_active": True, "auto_fetch_models": False,
    })
    return pid, key["id"]


def strategy(keep, remove):
    """生成有效与待删除实体混合的排序，保留规则的条件和停止语义。"""
    return {"rules": [{"id": "ui_provider_priority", "enabled": True, "stop_processing": True,
                       "actions": [
                           {"type": "set_provider_priority", "provider_id": remove[0], "priority": 1},
                           {"type": "set_provider_priority", "provider_id": keep[0], "priority": 2},
                           {"type": "set_key_priority", "key_id": remove[1], "priority": 3},
                           {"type": "set_key_priority", "key_id": keep[1], "priority": 4},
                       ]}]}


def delete_provider(session, pid):
    """等待后台删除真正完成，避免把提交任务当作完成。"""
    task = session.json("DELETE", f"/api/admin/providers/{pid}")
    for _ in range(200):
        state = session.json("GET", f"/api/admin/providers/{pid}/delete-task/{task['task_id']}")
        if state["status"] == "completed":
            return
        assert state["status"] != "failed", "供应商后台删除失败"
        time.sleep(0.1)
    raise AssertionError("供应商删除超时")


def assert_backup(gateway, keep, remove, binary):
    """下载完整备份到磁盘再解析，检查有效实体和优先级，随后在隔离实例恢复。"""
    with gateway.session.request("GET", "/api/admin/system/data/export") as response:
        assert response.status == 200
        content = response.read()
    output = gateway.root / "backup.json"
    output.write_bytes(content)
    output.chmod(0o600)
    backup = json.loads(output.read_bytes())
    config = backup["config_data"]
    assert isinstance(backup["user_data"], dict)
    entities = {p["id"]: p for p in config["providers"]}
    assert keep[0] in entities and remove[0] not in entities
    assert any(k["id"] == keep[1] for k in entities[keep[0]]["api_keys"])
    rules = config["routing_strategy"]["config_json"]["rules"]
    assert rules[0]["stop_processing"] is True
    actions = rules[0]["actions"]
    assert actions == strategy(keep, remove)["rules"][0]["actions"][1::2]
    result = gateway.session.json("POST", "/api/admin/system/data/import", {**backup, "merge_mode": "skip"})
    assert not result.get("stats", {}).get("errors"), "恢复返回错误"
    gateway.session.login()
    again = gateway.session.json("GET", "/api/admin/system/data/export")
    assert any(p["name"] == entities[keep[0]]["name"] for p in again["config_data"]["providers"])
    print("PASS: 完整导出 HTTP 200，文件可解析，有效供应商/密钥/优先级保留，恢复后可再次导出", flush=True)
    restored = Gateway()
    try:
        restored.start(binary)
        result = restored.session.json("POST", "/api/admin/system/data/import", {**backup, "merge_mode": "skip"})
        assert not result.get("stats", {}).get("errors"), "跨实例恢复返回错误"
        restored.session.login()
        document = restored.session.json("GET", "/api/admin/system/data/export")["config_data"]
        entity = next(p for p in document["providers"] if p["name"] == entities[keep[0]]["name"])
        assert len(entity["api_keys"]) == 1
        actions = document["routing_strategy"]["config_json"]["rules"][0]["actions"]
        assert actions == [
            {"type": "set_provider_priority", "provider_id": entity["id"], "priority": 2},
            {"type": "set_key_priority", "key_id": entity["api_keys"][0]["id"], "priority": 4},
        ]
        print("PASS: 备份恢复到另一份空数据库，重新导出的供应商/密钥优先级关联正确", flush=True)
    finally:
        restored.stop()


def scenario(binary, legacy=None):
    """先验证真实删除链路；可用旧版本留下原故障，再换新版验证升级修复。"""
    gateway = Gateway()
    try:
        gateway.start(legacy or binary)
        session = gateway.session
        keep = provider(session, "Keep local")
        remove = provider(session, "Remove local")
        original = strategy(keep, remove)
        group = session.json("POST", "/api/admin/routing/groups", {
            "name": "Backup regression", "is_system_default": True, "config_json": original,
        })
        delete_provider(session, remove[0])
        if legacy:
            try:
                session.json("GET", "/api/admin/system/data/export")
            except urllib.error.HTTPError as error:
                payload = json.load(error)
                assert error.code == 500 and "调度策略引用的 provider_id" in payload["error"]["message"]
                print("PASS: 旧版删除后复现同类 HTTP 500", flush=True)
            else:
                raise AssertionError("旧版未复现原故障")
            gateway.stop()
            gateway.start(binary)
            session = gateway.session
            print("PASS: 同一隔离数据库由新版启动完成迁移", flush=True)
        before_save = session.json("GET", f"/api/admin/routing/groups/{group['id']}")
        assert before_save["config_json"]["rules"][0]["actions"] == original["rules"][0]["actions"][1::2]
        # 模拟旧页面携带已删除实体重新保存，检查响应与后续读取一致。
        updated = session.json("PATCH", f"/api/admin/routing/groups/{group['id']}", {"config_json": original})
        current = session.json("GET", f"/api/admin/routing/groups/{group['id']}")
        assert updated["config_json"] == current["config_json"]
        assert_backup(gateway, keep, remove, binary)
        with sqlite3.connect(gateway.root / "test.db") as db:
            assert db.execute("SELECT count(*) FROM provider_api_keys WHERE id=?", (keep[1],)).fetchone()[0] == 1
        print("诊断和备份目录：", gateway.root, flush=True)
    finally:
        gateway.stop()


def main():
    """运行当前版本验收及可选旧版本升级验收，失败返回非零。"""
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("gateway")
    parser.add_argument("--legacy")
    args = parser.parse_args()
    scenario(args.gateway)
    if args.legacy:
        scenario(args.gateway, args.legacy)


if __name__ == "__main__":
    main()
