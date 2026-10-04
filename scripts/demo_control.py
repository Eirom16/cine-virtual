"""Run the localhost control spike with three actual executable processes."""

import json
import queue
import signal
import subprocess
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class Process:
    def __init__(self, args):
        self.process = subprocess.Popen(args, cwd=ROOT, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                        text=True, bufsize=1)
        self.output = queue.Queue()
        self.logs = []
        self.reader = threading.Thread(target=self.read_output, daemon=True)
        self.logger = threading.Thread(target=self.read_logs, daemon=True)
        self.reader.start()
        self.logger.start()

    def read_output(self):
        for line in self.process.stdout:
            self.output.put(json.loads(line))

    def read_logs(self):
        for line in self.process.stderr:
            try:
                self.logs.append(json.loads(line))
            except json.JSONDecodeError:
                self.logs.append({"non_json_error": line.strip()})

    def receive(self):
        return self.output.get(timeout=8)

    def command(self, command):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()
        return self.receive()

    def stop(self):
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.logger.join(timeout=1)


def wait_state(client, predicate):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        state = client.command("state")
        if predicate(state):
            return state
        time.sleep(0.02)
    raise AssertionError("Client state did not converge")


def execution(client, sequence):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        for record in client.logs:
            fields = record.get("fields", {})
            if fields.get("event") == "player_executed" and fields["sequence"] == sequence:
                return fields
        time.sleep(0.01)
    raise AssertionError("Scheduled player action did not execute")


def main():
    processes = []
    try:
        server = Process([str(ROOT / "target/debug/cine-server"), "--bind", "127.0.0.1:0"])
        processes.append(server)
        deadline = time.monotonic() + 5
        address = None
        while time.monotonic() < deadline:
            for record in server.logs:
                fields = record.get("fields", {})
                if fields.get("event") == "server_started":
                    address = fields["bind"]
            if address:
                break
            time.sleep(0.01)
        assert address, "Server did not start"
        clients = []
        for name in ("Host", "Participant"):
            client = Process([str(ROOT / "target/debug/cine-client"), "--server", "ws://" + address, "--name", name])
            processes.append(client)
            assert client.receive()["event"] == "cli_connected"
            clients.append(client)
        a, b = clients
        invitation = a.command("create")
        # Credentials remain only in private process memory and never enter the report.
        assert invitation["event"] == "created"
        join = "join {room_id} {room_epoch} {invite_token}".format(**invitation)
        assert b.command(join)["event"] == "joined"
        wait_state(a, lambda s: s["sequence"] == 2)
        assert a.command("media-demo")["event"] == "media_selected"
        wait_state(b, lambda s: s["sequence"] == 3)
        assert a.command("ready")["event"] == "ready"
        assert b.command("ready")["event"] == "ready"
        ready = wait_state(a, lambda s: all(m["ready"] for m in s["members"]))
        before = ready["sequence"]
        rejected = b.command("play 100000")
        assert rejected == {"event": "command_error", "message": "NOT_AUTHORIZED"}
        assert a.command("state")["sequence"] == before
        metrics = []
        for command in ("play 100000", "pause", "seek 120000"):
            accepted = a.command(command)
            assert accepted["event"] == "accepted", accepted
            seq = accepted["sequence"]
            ea, eb = execution(a, seq), execution(b, seq)
            assert ea["expected_server_ms"] == eb["expected_server_ms"]
            difference = abs(ea["actual_server_ms"] - eb["actual_server_ms"])
            assert difference < 150, difference
            assert abs(ea["lateness_ms"]) < 200 and abs(eb["lateness_ms"]) < 200
            metrics.append({"command": command, "sequence": seq, "execute_at_ms": ea["expected_server_ms"],
                            "lateness_a_ms": ea["lateness_ms"], "lateness_b_ms": eb["lateness_ms"],
                            "execution_difference_ms": difference})
        before = a.command("state")["sequence"]
        assert b.command("disconnect")["event"] == "disconnected"
        wait_state(a, lambda s: s["sequence"] > before and not s["members"][1]["connected"])
        seq = a.command("play 120000")["sequence"]
        execution(a, seq)
        assert b.command("resume")["event"] == "resumed"
        sa = wait_state(a, lambda s: s["members"][1]["ready"])
        sb = b.command("sync")
        assert sa["sequence"] == sb["sequence"] and sa["room_id"] == sb["room_id"]
        assert sa["playing"] and sb["playing"]
        drift = abs(sa["position_ms"] - sb["position_ms"])
        assert drift < 150, drift
        seq = a.command("pause")["sequence"]
        execution(a, seq)
        execution(b, seq)
        final_a, final_b = a.command("state"), b.command("state")
        assert not final_a["playing"] and not final_b["playing"]
        assert final_a["position_ms"] == final_b["position_ms"]
        for client in clients:
            assert client.command("quit")["event"] == "quit"
            client.process.wait(timeout=3)
        for process in processes:
            process.stop()
        forbidden = ("invite_token", "resume_token", "digest", "payload")
        assert all(not any(key in record.get("fields", {}) for key in forbidden)
                   for process in processes for record in process.logs)
        report = {"scenario": "three_real_processes", "result": "PASS", "authority": "NOT_AUTHORIZED; sequence unchanged",
                  "timings": metrics, "clock_a": final_a["clock"], "clock_b": final_b["clock"],
                  "resume_position_difference_ms": drift, "final_sequence": final_a["sequence"],
                  "final_position_ms": final_a["position_ms"], "tokens_in_structured_logs": False}
        print(json.dumps(report, ensure_ascii=False, indent=2))
    finally:
        for process in reversed(processes):
            process.stop()


if __name__ == "__main__":
    main()
