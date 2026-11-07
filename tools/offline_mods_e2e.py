#!/usr/bin/env python3
"""Exercise TreeFall and Mouse Tricks through a real isolated game and ordinary player input."""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import tempfile
import time

from offline_skin_e2e import Peer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--launcher', type=lambda value: Path(value).resolve())
    group.add_argument('--bin-dir', type=lambda value: Path(value).resolve())
    parser.add_argument('--resources', required=True, type=lambda value: Path(value).resolve())
    parser.add_argument('--out', required=True, type=lambda value: Path(value).resolve())
    parser.add_argument('--headless', action='store_true')
    args = parser.parse_args()
    args.mods = bool(args.launcher)
    args.out.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='optional-mods-', dir=args.out))
    print(f'Scratch output: {root}', flush=True)
    peer = Peer(root, 'host', 'ModsHost', None, False, args, args.resources)
    report = dict(microsoft_accounts=0, headless=args.headless)
    try:
        peer.start()
        x, y, z = map(math.floor, peer.call('state')['player']['position'])
        def command(text):
            reply = peer.call('command', text)
            assert reply.get('ok'), reply
            return reply
        command('time set day')
        command('gamerule spawn_mobs false')
        status = command('treefall status')
        assert any('TreeFall' in line for line in status.get('chat', [])), status
        command(f'fill {x-8} {y-1} {z-8} {x+8} {y-1} {z+8} minecraft:stone')
        command(f'fill {x-8} {y} {z-8} {x+8} {y+9} {z+8} minecraft:air')
        tx = x + 3
        command(f'fill {tx-1} {y+3} {z-1} {tx+1} {y+4} {z+1} minecraft:oak_leaves[persistent=false,distance=1]')
        command(f'fill {tx} {y} {z} {tx} {y+3} {z} minecraft:oak_log')
        command('item replace entity ModsHost weapon.mainhand with minecraft:diamond_axe')
        command('gamemode survival ModsHost')
        peer.call('wait', 20)
        peer.call('break', tx, y, z, '--timeout', 20)
        assert not peer.call('blocks', tx, y, z, tx, y+3, z)['blocks'], 'one cut must remove all four logs'
        peer.call('tp', tx + 0.5, y, z + 0.5)
        peer.call('wait', 60)
        inventory = peer.call('inventory')
        assert inventory['counts'].get('minecraft:oak_log') == 4, inventory
        assert inventory['held']['item'] == 'minecraft:diamond_axe' and inventory['held']['damage'] == 4, inventory
        report['treefall'] = dict(logs_felled=4, logs_received=4, axe_damage=4)

        for slot, item, count in [(0, 'dirt', 8), (1, 'stone', 3), (2, 'dirt', 11), (3, 'stone', 7)]:
            command(f'item replace entity ModsHost inventory.{slot} with minecraft:{item} {count}')
        peer.call('open-inventory')
        peer.call('key', 'r', 'tap')
        peer.call('wait', 40)
        inventory = peer.call('inventory')
        stacks = [slot for slot in inventory['main'] if slot.get('item')]
        assert len(stacks) == 2 and Counter({slot['item']: slot['count'] for slot in stacks}) == Counter({'minecraft:stone': 10, 'minecraft:dirt': 19}), inventory
        assert inventory['carried'] is None, inventory
        report['mousetricks_inventory'] = dict(stone=10, dirt=19, stacks=2, carried=None)
        peer.call('close')

        if not args.headless:
            cx, cz = x + 1, z + 2
            command(f'setblock {cx} {y} {cz} minecraft:chest')
            for slot, item, count in [(0, 'dirt', 8), (1, 'stone', 3), (2, 'dirt', 11), (3, 'stone', 7)]:
                command(f'item replace block {cx} {y} {cz} container.{slot} with minecraft:{item} {count}')
            peer.call('interact', cx, y, cz)
            peer.call('wait', 3)
            ui = peer.call('ui')
            widgets = ui if isinstance(ui, list) else ui.get('widgets', ui.get('ui', []))
            slot = next(widget for widget in widgets if widget.get('id') == 'slot-304')
            left, top, width, height = slot['bounds']
            # click-slot is a semantic shortcut; coordinates also move the actual pointer.
            peer.call('click-at', left + width/2, top + height/2)
            peer.call('key', 'r', 'tap')
            peer.call('wait', 40)
            slots = [slot for slot in peer.call('container', cx, y, cz)['slots'] if slot]
            assert len(slots) == 2 and {slot['item']: slot['count'] for slot in slots} == {'minecraft:stone': 10, 'minecraft:dirt': 19}, slots
            peer.call('screenshot', root / 'mouse-tricks-chest.png')
            report['mousetricks_chest'] = dict(stone=10, dirt=19, stacks=2)
            peer.call('close')
            peer.call('options', 'mods')
            deadline = time.monotonic() + 15
            while True:
                listed = json.dumps(peer.call('ui'), ensure_ascii=False)
                if 'Mouse Tricks' in listed and 'TreeFall' in listed:
                    break
                assert time.monotonic() < deadline, listed
                time.sleep(0.05)
            peer.call('screenshot', root / 'optional-mod-settings.png')
            report['settings_list_both_mods'] = True
        report['passed'] = True
    finally:
        peer.close()
    (root / 'result.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('PASS: TreeFall fells four logs with correct drops and wear; Mouse Tricks sorts without item loss.', flush=True)


if __name__ == '__main__':
    main()
