#!/usr/bin/env python3
"""Exercise real file reads and GTK events through AIVI's MCP server.

Requires a graphical desktop session and target/debug/aivi. All editable input is
copied to a temporary directory; the checked-in inventory is never modified.
"""
import json
import os
import pathlib
import selectors
import shutil
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[2]
DEMO = pathlib.Path(__file__).resolve().parent


class App:
    def __init__(self, work, stderr, inventory_path=None):
        environment = {key: value for key, value in os.environ.items() if key != "AIVI_STOCKROOM_FILE"}
        if inventory_path is not None:
            environment["AIVI_STOCKROOM_FILE"] = str(inventory_path)
        self.process = subprocess.Popen(
            [str(ROOT / "target/debug/aivi"), "mcp", "--path", str(work / "main.aivi")],
            env=environment,
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=stderr, text=True, bufsize=1,
        )
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.next_id = 0
        try:
            self.request("initialize", {
                "protocolVersion": "2024-11-05", "capabilities": {},
                "clientInfo": {"name": "stockroom-smoke", "version": "1"},
            })
            self.call("launch_app", {"cwd": str(work)})
        except Exception:
            self.process.terminate()
            self.process.wait(timeout=3)
            self.process.stdin.close()
            self.process.stdout.close()
            self.selector.close()
            raise

    def request(self, method, params):
        self.next_id += 1
        self.process.stdin.write(json.dumps({
            "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params,
        }) + "\n")
        self.process.stdin.flush()
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            if not self.selector.select(max(0, deadline - time.monotonic())):
                break
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("AIVI MCP exited unexpectedly")
            response = json.loads(line)
            if response.get("id") == self.next_id:
                if "error" in response:
                    raise RuntimeError(response["error"])
                return response["result"]
        raise TimeoutError(f"AIVI MCP timed out: {method}")

    def call(self, name, arguments=None):
        result = self.request("tools/call", {"name": name, "arguments": arguments or {}})
        if result.get("isError"):
            raise RuntimeError(result)
        return result.get("structuredContent") or json.loads(result["content"][0]["text"])

    def signals(self):
        return {signal["name"]: signal["value"] for signal in self.call("list_signals")["signals"]}

    def wait_for(self, predicate):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            signals = self.signals()
            if predicate(signals):
                return signals
            time.sleep(0.05)
        raise AssertionError(f"Expected state was not reached: {self.signals()}")

    def widget(self, kind, text=None):
        args = {"kind": kind}
        if text is not None:
            args["text_contains"] = text
        widgets = self.call("find_widgets", args)["widgets"]
        assert len(widgets) == 1, widgets
        return widgets[0]["id"]

    def search(self, text):
        self.call("emit_gtk_event", {
            "widget_id": self.widget("GtkSearchEntry"), "event": "set_text", "text": text,
        })

    def reload(self):
        before = next(s["generation"] for s in self.call("list_signals")["signals"] if s["name"] == "inventory")
        self.call("emit_gtk_event", {
            "widget_id": self.widget("GtkButton", "Reload"), "event": "click",
        })

        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            signals = self.call("list_signals")["signals"]
            current = next(s["generation"] for s in signals if s["name"] == "inventory")
            loading = next(s["value"] for s in signals if s["name"] == "inventory#loading")
            if current > before and loading is False:
                return
            time.sleep(0.05)
        raise AssertionError(f"Reload did not publish a new inventory result: {self.call('session_status')} {self.signals()}")

    def screenshot(self, name):
        capture = self.call("capture_gtk_screenshot")["captures"][0]
        target = ROOT / "out" / name
        target.parent.mkdir(exist_ok=True)
        shutil.copyfile(capture["path"], target)
        print(f"Screenshot: {target}")

    def close(self):
        try:
            if self.process.poll() is None:
                self.call("stop_app")
        finally:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=3)
            self.process.stdout.close()
            self.selector.close()


def main():
    with tempfile.TemporaryDirectory(prefix="aivi-stockroom-") as directory:
        work = pathlib.Path(directory)
        for path in DEMO.glob("*.aivi"):
            shutil.copyfile(path, work / path.name)
        original = json.loads((DEMO / "inventory.json").read_text())
        data = work / "inventory.json"
        data.write_text(json.dumps(original))
        with (work / "stderr.log").open("w+") as stderr:
            app = None
            try:
                app = App(work, stderr)
                state = app.wait_for(lambda s: s["screen"]["tag"] == "Ready")
                assert [(r["sku"], r["quantity"]) for r in state["plan"]] == [
                    ("PAPER-A4", 10), ("SOAP", 10), ("COFFEE", 3),
                ]
                app.screenshot("stockroom.png")
                app.search("  CoFfEe ")
                app.wait_for(lambda s: len(s["plan"]) == 1 and s["plan"][0]["sku"] == "COFFEE")
                updated = json.loads(json.dumps(original))
                updated["items"][1]["target"] = 7
                data.write_text(json.dumps(updated))
                app.reload()
                state = app.wait_for(lambda s: len(s["plan"]) == 1 and s["plan"][0]["quantity"] == 6)
                assert state["query"] == "  CoFfEe "
                app.search("no such item")
                app.wait_for(lambda s: s["plan"] == [])
                assert app.call("find_widgets", {"text_contains": "No replenishment items"})["widgets"]
                app.search("")

                for payload, detail in [
                    ('{"items":[{"sku":"BAD","name":"Bad","onHand":0,"target":"ten"}]}', "Invalid inventory data"),
                    ('{"items":[{"sku":"BAD","name":"Bad","onHand":-1,"target":10}]}', "nonnegative"),
                    ('{"items":[],"unexpected":true}', "Invalid inventory data"),
                    ('{"items":[{"sku":"BAD","name":"Bad","onHand":0}]}', "Invalid inventory data"),
                ]:
                    data.write_text(payload)
                    app.reload()
                    state = app.wait_for(lambda s: s["screen"]["tag"] == "Failed" and detail in s["screen"]["payload"])
                    assert detail in state["screen"]["payload"], state
                    assert state["plan"] == [], state
                data.unlink()
                app.reload()
                state = app.wait_for(lambda s: s["screen"]["tag"] == "Failed" and "Inventory file not found" in s["screen"]["payload"])
                assert str(data) in state["screen"]["payload"]
                data.write_text(json.dumps(original))
                app.reload()
                app.wait_for(lambda s: s["screen"]["tag"] == "Ready" and len(s["plan"]) == 3)
                assert app.call("session_status")["session"]["runtime_error"] is None
                app.close()
                app = None

                source = work / "main.aivi"
                source.write_text(source.read_text().replace("defaultWidth={640}", "defaultWidth={360}"))
                external = work / "external-inventory.json"
                external.write_text(json.dumps(original))
                data.write_text("invalid JSON to verify the external override is used")
                app = App(work, stderr, inventory_path=external)
                app.wait_for(lambda s: s["screen"]["tag"] == "Ready" and len(s["plan"]) == 3)
                app.screenshot("stockroom-360.png")
                print("Stockroom GTK smoke passed: file decode, search, reload, schema/domain failures, recovery, external path, narrow launch.")
            except Exception:
                stderr.flush()
                stderr.seek(0)
                print(stderr.read())
                raise
            finally:
                if app is not None:
                    app.close()


if __name__ == "__main__":
    main()
