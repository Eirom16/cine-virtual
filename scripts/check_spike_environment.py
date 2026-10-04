"""Check network prerequisites for the local WebSocket spike."""

import argparse
import json
import socket
from urllib.request import urlopen


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true", help="Skip DNS/HTTPS when dependencies are cached")
    args = parser.parse_args()
    results = []
    checks = ("localhost_tcp",) if args.offline else ("localhost_tcp", "crates_io_dns", "crates_io_https")
    for check in checks:
        try:
            if check == "localhost_tcp":
                with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
                    listener.bind(("127.0.0.1", 0))
                    listener.listen(1)
            elif check == "crates_io_dns":
                socket.getaddrinfo("index.crates.io", 443, type=socket.SOCK_STREAM)
            else:
                with urlopen("https://index.crates.io/config.json", timeout=5) as response:
                    if response.status != 200:
                        raise OSError("crates.io returned an unexpected status")
            results.append({"check": check, "ok": True})
        except OSError as error:
            results.append({"check": check, "ok": False, "error": str(error)})
    print(json.dumps({"checks": results}, ensure_ascii=False, indent=2))
    return 0 if all(result["ok"] for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
