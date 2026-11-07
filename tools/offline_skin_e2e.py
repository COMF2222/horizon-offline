#!/usr/bin/env python3
"""Three real offline clients exchange local skins over an isolated loopback LAN.

CI: python tools/offline_skin_e2e.py --launcher <embedded.exe> --out <temp> --headless
Developer rerun: --bin-dir <matching game binaries> --resources <prepared resources>.
Without --headless, offscreen clients also save screenshots for visual inspection.
All homes are freshly created; private downloaded/prepared files stay in that scratch tree.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import struct
import subprocess
import tempfile
import time
import zlib


def hidden_process_options():
    if os.name != 'nt':
        return {'start_new_session': True}
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    return {'startupinfo': startup, 'creationflags': subprocess.CREATE_NO_WINDOW}


def stop_tree(process):
    if process.poll() is not None:
        return
    if os.name == 'nt':
        subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                       capture_output=True, check=False)
    else:
        os.killpg(process.pid, signal.SIGTERM)
    process.wait(timeout=20)


def fixture(path, rgb):
    """A synthetic 64×64 skin, with no Mojang image data."""
    rgba = bytes((*rgb, 255)) * (64 * 64)
    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))
    rows = b''.join(b'\0' + rgba[i:i + 256] for i in range(0, len(rgba), 256))
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 64, 64, 8, 6, 0, 0, 0))
                     + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
    return hashlib.sha1(rgba).hexdigest()


def link_file(source, target):
    try:
        os.link(source, target)
    except OSError:
        shutil.copy2(source, target)


class Peer:
    def __init__(self, root, role, name, color, slim, args, resources=None):
        self.root = root / role
        self.root.mkdir()
        self.name, self.slim, self.args = name, slim, args
        self.process = None
        # Ignore any profile, session, fixture or data path inherited from a developer's shell.
        self.env = {key: value for key, value in os.environ.items() if not key.startswith('HORIZON_')}
        for key, folder in [('HORIZON_HOME', 'game'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                            ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config'),
                            ('XDG_CACHE_HOME', 'cache'), ('XDG_RUNTIME_DIR', 'run'),
                            ('HORIZON_LAUNCHER_TRASH', 'trash'), ('HORIZON_LAUNCHER_CLIPBOARD', 'clipboard'),
                            ('HORIZON_LAUNCHER_DESKTOP', 'desktop')]:
            path = self.root / folder
            path.mkdir()
            self.env[key] = str(path)
        self.endpoint = self.root / 'control.json'
        self.env.update(HORIZON_CONTROL=str(self.endpoint), HORIZON_LAUNCHER_NO_SELF_UPDATE='1',
                        HORIZON_NO_AUDIO='1', HORIZON_NO_KEYCHAIN='1', HORIZON_TELEMETRY_URL='off',
                        HORIZON_TEST_LOOPBACK='1', HORIZON_STUN_SERVERS='none')
        self.skin = self.root / 'synthetic-skin.png'
        self.digest = fixture(self.skin, color) if color else None
        if args.launcher:
            self.run_launcher('account', 'offline', name)
            if color:
                self.run_launcher('account', 'skin', name, str(self.skin), '--model', 'slim' if slim else 'classic')
            accounts = json.loads((self.root / 'launcher/accounts.json').read_text(encoding='utf-8'))
            assert not accounts['accounts'], 'the test must never use a Microsoft account'
            settings = json.loads((self.root / 'launcher/settings.json').read_text(encoding='utf-8'))
            selected = settings.get('selected')
            assert selected, 'the supplied launcher must embed and select its own runtime'
            self.runtime = self.root / 'launcher/versions' / selected
            lock = json.loads((self.runtime / 'research-lock.json').read_text(encoding='utf-8'))
            self.resources = self.runtime / '.local/horizon-client' / lock['minecraft']['version']
            if resources:
                self.resources.parent.mkdir(parents=True, exist_ok=True)
                shutil.copytree(resources, self.resources, copy_function=link_file, dirs_exist_ok=True)
            else:
                self.run_launcher('prepare', '--download', timeout=1800)
            self.bin_dir = self.runtime
        else:
            self.bin_dir = args.bin_dir
            self.resources = resources
            if color:
                self.env.update(HORIZON_OFFLINE_SKIN=str(self.skin),
                                HORIZON_OFFLINE_SKIN_MODEL='slim' if slim else 'classic')
        assert self.resources and (self.resources / 'horizon-resources.json').is_file(), self.resources
        suffix = '.exe' if os.name == 'nt' else ''
        self.ctl = self.bin_dir / f'horizon-ctl{suffix}'
        self.game = self.bin_dir / f'horizon-client{suffix}'
        assert self.ctl.is_file() and self.game.is_file(), self.bin_dir

    def run_launcher(self, *words, timeout=300):
        label = '-'.join(words[:2]).replace('/', '_')
        log = self.root / f'{label}.log'
        with log.open('wb') as output:
            process = subprocess.Popen([str(self.args.launcher), *words], cwd=self.root, env=self.env,
                                       stdout=output, stderr=subprocess.STDOUT, **hidden_process_options())
            try:
                code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                stop_tree(process)
                raise
        assert code == 0, f'{self.name}: launcher {words[:2]} failed; see {log}'

    def start(self, address=None):
        mode = '--headless' if self.args.headless else '--offscreen'
        options = [mode, '--control', '--control-socket', str(self.endpoint), '--no-pause',
                   '--render-distance', '4']
        if self.args.launcher:
            command = [str(self.args.launcher), 'play']
            command += ['--server', address] if address else ['--world', 'SkinLAN', '--creative']
            command += [f'--arg={word}' for word in options]
        else:
            command = [str(self.game), '--resources', str(self.resources), '--game-dir', str(self.root / 'game'),
                       '--name', self.name, *options]
            command += ['--server', address] if address else ['--world', 'SkinLAN', '--creative', '--seed', '5']
        # Ready resources must play without any reachable remote download service.
        env = dict(self.env, HTTPS_PROXY='http://127.0.0.1:9', HTTP_PROXY='http://127.0.0.1:9',
                   ALL_PROXY='http://127.0.0.1:9', NO_PROXY='localhost,127.0.0.1')
        with (self.root / 'game.log').open('wb') as output:
            self.process = subprocess.Popen(command, cwd=self.root / 'game', env=env,
                                            stdout=output, stderr=subprocess.STDOUT, **hidden_process_options())
        deadline = time.monotonic() + 180
        while not self.endpoint.is_file():
            assert self.process.poll() is None, f'{self.name} exited; see {self.root / "game.log"}'
            assert time.monotonic() < deadline, f'{self.name} did not create its control endpoint'
            time.sleep(0.2)
        self.call('wait-for', 'loaded', '--timeout', '300', timeout=330)

    def call(self, *words, timeout=60):
        result = subprocess.run([str(self.ctl), '--socket', str(self.endpoint), *map(str, words)],
                                cwd=self.root, env=self.env, capture_output=True, text=True,
                                encoding='utf-8', errors='replace', timeout=timeout, **hidden_process_options())
        lines = result.stdout.strip().splitlines()
        reply = json.loads(lines[-1]) if lines else {'error': result.stderr.strip()}
        assert result.returncode == 0 and 'error' not in reply, f'{self.name} {words}: {reply}'
        return reply

    def skins(self):
        return {entry['name']: entry for entry in self.call('status')['server']['offline_skins']}

    def wait_skins(self, expected, players):
        deadline = time.monotonic() + 60
        while True:
            status = self.call('status')['server']
            entries = {entry['name']: entry for entry in status['offline_skins']}
            okay = set(status['players']) == set(players) and set(entries) == set(expected)
            if okay:
                for name, peer in expected.items():
                    entry = entries[name]
                    assert entry['slim'] == peer.slim, entry
                    assert entry['texture'].endswith('/' + peer.digest), entry
                return entries
            assert time.monotonic() < deadline, f'{self.name}: missing skin/profile update: {status}'
            time.sleep(0.2)

    def close(self):
        if self.process is None:
            return
        if self.process.poll() is None:
            try:
                self.call('quit', timeout=20)
                self.process.wait(timeout=30)
            except Exception:
                stop_tree(self.process)
        assert self.process.returncode == 0, f'{self.name} exited unsuccessfully; see {self.root / "game.log"}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    binary = parser.add_mutually_exclusive_group(required=True)
    binary.add_argument('--launcher', type=lambda value: Path(value).resolve())
    binary.add_argument('--bin-dir', type=lambda value: Path(value).resolve())
    parser.add_argument('--resources', type=lambda value: Path(value).resolve())
    parser.add_argument('--out', type=lambda value: Path(value).resolve(), default=Path(tempfile.gettempdir()))
    parser.add_argument('--headless', action='store_true', help='no GPU; verify decoded peer skins, without screenshots')
    args = parser.parse_args()
    if args.bin_dir and not args.resources:
        parser.error('--bin-dir requires matching --resources')
    args.out.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='offline-skins-', dir=args.out))
    print(f'Scratch output: {root}', flush=True)
    peers, report = [], {}
    try:
        host = Peer(root, 'host', 'SkinHost', (232, 40, 24), False, args, args.resources)
        peers.append(host)
        host.start()
        host.call('command', 'time set day')
        host.call('command', 'gamerule spawn_mobs false')
        host.call('command', 'fill ~-8 ~-1 ~-8 ~8 ~-1 ~8 minecraft:stone')
        host.call('command', 'fill ~-8 ~ ~-8 ~8 ~6 ~8 minecraft:air')
        address = f'127.0.0.1:{host.call("lan", "open", "0", "commands")["port"]}'
        blue = Peer(root, 'blue', 'SkinBlue', (24, 40, 232), True, args, host.resources)
        peers.append(blue)
        blue.start(address)
        expected = {peer.name: peer for peer in peers}
        for peer in peers:
            peer.wait_skins(expected, expected)
        green = Peer(root, 'late-green', 'SkinGreen', (24, 232, 40), False, args, host.resources)
        peers.append(green)
        green.start(address)
        expected = {peer.name: peer for peer in peers}
        report['late_join'] = {peer.name: peer.wait_skins(expected, expected) for peer in peers}
        if not args.headless:
            x, y, z = host.call('state')['player']['position']
            host.call('command', f'gamemode creative {blue.name}')
            host.call('command', f'tp {blue.name} {x + 3} {y} {z}')
            host.call('look-at', x + 3, y + 1.3, z)
            blue.call('look-at', x, y + 1.3, z)
            for observer, subject in [(host, blue), (blue, host)]:
                observer.call('wait', 20)
                observer.call('screenshot', str(root / f'{observer.name}-sees-{subject.name}.png'))
                entry = observer.skins()[subject.name]
                assert entry['atlas_ready'] and entry['model_uses_skin'], entry
            report['rendered_models'] = True
        green.close()
        expected = {peer.name: peer for peer in (host, blue)}
        for peer in (host, blue):
            peer.wait_skins(expected, expected)
        rejoined = Peer(root, 'green-default', 'SkinGreen', None, False, args, host.resources)
        peers.append(rejoined)
        rejoined.start(address)
        report['default_on_rejoin'] = {peer.name: peer.wait_skins(expected, [*expected, 'SkinGreen'])
                                      for peer in (host, blue, rejoined)}
        report.update(passed=True, microsoft_accounts=0, headless=args.headless)
    finally:
        failures = []
        for peer in reversed(peers):
            try:
                peer.close()
            except Exception as error:
                failures.append(str(error))
        if failures:
            raise AssertionError('; '.join(failures))
    (root / 'result.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('PASS: three local profiles, late join, classic/slim skins, disconnect and default-skin rejoin.', flush=True)


if __name__ == '__main__':
    main()
