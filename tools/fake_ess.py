#!/usr/bin/env python3
"""Local stand-in for the PlanetSide 2 event stream (ESS), for end-to-end tests.

Debug builds connect to it with PS2_TRACKER_ESS_URL=ws://127.0.0.1:8765
(release builds ignore the variable). After the subscription it sends a
PlayerLogin per character and then N Death events in which the attackers
change, so the squad ranking moves; every fourth event a tracked character
dies instead. Only synthetic IDs, nothing from a real account.

    python3 tools/fake_ess.py --own 5428000000000000042 \
        --squad 5428000000000000077 5428000000000000078 --kills 20 --interval 1

Needs the Python package "websockets" (>= 13).
"""

import argparse
import asyncio
import json
import time

from websockets.asyncio.server import serve

FOE = 5429000000000000001
# real loadout IDs (Census ps2:v2/loadout): NC classes for the tracked side,
# VS classes for the enemies, so the kill feed shows changing classes
OWN_LOADOUTS = [6, 4, 5, 1, 3, 7]  # NC HA, Medic, Engineer, Infiltrator, LA, MAX
FOE_LOADOUTS = [18, 20, 15, 17, 19, 21]  # VS Medic, HA, Infiltrator, LA, Engineer, MAX


def message(payload: dict) -> str:
    payload = {k: str(v) for k, v in payload.items()}
    return json.dumps({"payload": payload, "service": "event", "type": "serviceMessage"})


def death(attacker: int, victim: int, world: int, zone: int, n: int) -> str:
    return message(
        {
            "event_name": "Death",
            "timestamp": int(time.time()),
            "world_id": world,
            "zone_id": zone,
            "character_id": victim,
            "attacker_character_id": attacker,
            "attacker_weapon_id": 80,
            "attacker_vehicle_id": 0,
            "attacker_loadout_id": OWN_LOADOUTS[n % 6] if victim == FOE + n else FOE_LOADOUTS[n % 6],
            "character_loadout_id": FOE_LOADOUTS[n % 6] if victim == FOE + n else OWN_LOADOUTS[n % 6],
            "attacker_fire_mode_id": 1,
            "is_critical": 0,
            "is_headshot": 1 if n % 3 == 0 else 0,
            "team_id": 2 if victim == FOE + n else 1,
            "attacker_team_id": 1 if victim == FOE + n else 2,
        }
    )


async def run(args: argparse.Namespace) -> None:
    tracked = [args.own, *args.squad]

    async def handler(ws):
        sub = await ws.recv()
        print("subscription:", sub[:200], flush=True)

        async def later():
            # live changes (subscribe/clearSubscribe) on the open connection
            async for msg in ws:
                print("received:", str(msg)[:300], flush=True)

        reader = asyncio.create_task(later())  # reference kept for the connection's lifetime
        await ws.send(json.dumps({"connected": "true", "service": "push", "type": "connectionStateChanged"}))
        for c in tracked:
            await ws.send(
                message(
                    {
                        "event_name": "PlayerLogin",
                        "timestamp": int(time.time()),
                        "world_id": args.world,
                        "character_id": c,
                    }
                )
            )
        for n in range(args.kills):
            await asyncio.sleep(args.interval)
            # attackers rotate with a growing weight, so the leader changes
            who = tracked[(n * 7 + n // 5) % len(tracked)]
            if n % 4 == 3:
                frame = death(FOE + n, who, args.world, args.zone, n)
                print(f"{n + 1:2}: {who} died", flush=True)
            else:
                frame = death(who, FOE + n, args.world, args.zone, n)
                print(f"{n + 1:2}: {who} killed", flush=True)
            await ws.send(frame)
        print("done, keeping the connection open", flush=True)
        while True:
            await asyncio.sleep(20)
            await ws.send(json.dumps({"online": {}, "service": "event", "type": "heartbeat"}))

    async with serve(handler, "127.0.0.1", args.port):
        await asyncio.get_running_loop().create_future()


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--own", type=int, required=True)
    p.add_argument("--squad", type=int, nargs="*", default=[])
    p.add_argument("--kills", type=int, default=20)
    p.add_argument("--interval", type=float, default=1.0)
    p.add_argument("--world", type=int, default=10)
    p.add_argument("--zone", type=int, default=2)
    p.add_argument("--port", type=int, default=8765)
    asyncio.run(run(p.parse_args()))


if __name__ == "__main__":
    main()
