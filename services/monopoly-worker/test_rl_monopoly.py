from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import textwrap
import time
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import monopoly_controller as controller  # noqa: E402
import rl_monopoly_runner as runner  # noqa: E402
from shared_game import SharedGame  # noqa: E402


AGENT_PROGRAM = r"""
import json, sys, time
mode = sys.argv[1]
print(json.dumps({"ready": True}), flush=True)
for raw in sys.stdin:
    request = json.loads(raw)
    print("student diagnostic", file=sys.stderr, flush=True)
    if mode == "timeout":
        time.sleep(3)
    elif mode == "crash":
        raise RuntimeError("boom")
    elif mode == "memory":
        try:
            bytearray(3 * 1024**3)
        except MemoryError:
            print(json.dumps({"error": "MemoryError"}), flush=True)
    elif mode == "illegal":
        print(json.dumps({"action": 999999}), flush=True)
    elif mode == "verbose":
        print("x" * 100000, file=sys.stderr, flush=True)
        print(json.dumps({"action": request["allowed_actions"][0]}), flush=True)
    else:
        print(json.dumps({"action": request["allowed_actions"][0]}), flush=True)
"""


class AgentProtocolTests(unittest.TestCase):
    def process(self, mode: str):
        temporary = tempfile.TemporaryDirectory()
        path = Path(temporary.name) / "agent.py"
        path.write_text(textwrap.dedent(AGENT_PROGRAM))
        process = runner.AgentProcess([sys.executable, str(path), mode])
        self.addCleanup(temporary.cleanup)
        self.addCleanup(process.close)
        return process

    def test_valid_decision_and_private_log(self):
        process = self.process("valid")
        self.assertEqual(process.choose_action({"decision_seed": 7}, 0, [4, 8]), 4)
        time.sleep(0.02)
        self.assertIn("student diagnostic", process._stderr_tail())

    def test_runtime_log_tail_is_bounded_while_stderr_is_drained(self):
        process = self.process("verbose")
        self.assertEqual(process.choose_action({}, 0, [1, 2]), 1)
        time.sleep(0.02)
        self.assertLessEqual(len(process._stderr_tail().encode()), 4_000)

    def test_real_adapter_loads_contract_and_uses_submission_workdir(self):
        with tempfile.TemporaryDirectory() as temporary:
            submission = Path(temporary)
            (submission / "marker.txt").write_text("ready")
            (submission / "agent.py").write_text(
                "from pathlib import Path\n"
                "def choose_action(state, player_id, allowed_actions):\n"
                "    assert Path('marker.txt').read_text() == 'ready'\n"
                "    assert state['schema_version'] == 'exposure-agent-v1'\n"
                "    return allowed_actions[-1]\n"
            )
            environment = os.environ.copy()
            environment.update(
                {
                    "EXPOSURE_SUBMISSION_DIR": str(submission),
                    "EXPOSURE_AGENT_PATH": "agent.py",
                }
            )
            process = runner.AgentProcess(
                [sys.executable, "-I", str(ROOT / "agent_image" / "runner.py")],
                environment=environment,
                cwd=submission,
            )
            try:
                action = process.choose_action(
                    {"schema_version": "exposure-agent-v1"}, 2, [4, 8]
                )
                self.assertEqual(action, 8)
            finally:
                process.close()

    def test_colab_agent_environment_does_not_inherit_worker_credentials(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "submission").mkdir()
            (root / "venv" / "bin").mkdir(parents=True)
            with (
                mock.patch.dict(
                    os.environ,
                    {"WORKER_TOKEN": "must-not-leak", "MONOPOLY_SITE": "secret-site"},
                ),
                mock.patch.object(runner, "AgentProcess", return_value=object()) as process,
            ):
                runner.venv_agent(root, "agent.py")
            environment = process.call_args.kwargs["environment"]
            self.assertNotIn("WORKER_TOKEN", environment)
            self.assertNotIn("MONOPOLY_SITE", environment)
            self.assertEqual(environment["HOME"], str(root / "agent-home"))
            self.assertEqual(environment["EXPOSURE_AGENT_PATH"], "agent.py")

    def test_startup_timeout_is_bounded_and_process_is_reaped(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "silent.py"
            path.write_text("import time; time.sleep(5)\n")
            started = time.monotonic()
            with (
                mock.patch.object(runner, "STARTUP_TIMEOUT_SECONDS", 0.05),
                self.assertRaisesRegex(runner.AgentCallError, "exceeded"),
            ):
                runner.AgentProcess([sys.executable, str(path)])
            self.assertLess(time.monotonic() - started, 1.0)

    def test_timeout_is_hard_two_seconds(self):
        process = self.process("timeout")
        started = time.monotonic()
        with self.assertRaisesRegex(runner.AgentCallError, "exceeded") as raised:
            process.choose_action({}, 0, [1, 2])
        elapsed = time.monotonic() - started
        self.assertEqual(raised.exception.kind, "timeout")
        self.assertGreaterEqual(elapsed, 1.9)
        self.assertLess(elapsed, 2.7)

    def test_crash_and_illegal_output_are_rejected(self):
        crash = self.process("crash")
        with self.assertRaises(runner.AgentCallError):
            crash.choose_action({}, 0, [1, 2])
        illegal = self.process("illegal")
        with self.assertRaisesRegex(runner.AgentCallError, "illegal action") as raised:
            illegal.choose_action({}, 0, [1, 2])
        self.assertEqual(raised.exception.kind, "illegal")

    def test_process_memory_limit_blocks_three_gib_allocation(self):
        process = self.process("memory")
        with self.assertRaisesRegex(runner.AgentCallError, "MemoryError"):
            process.choose_action({}, 0, [1, 2])

    def test_third_strike_replaces_submitted_agent(self):
        runtime = runner.SeatRuntime(
            seat=1,
            label="Team",
            entry_id="entry",
            bot_key=None,
            artifact_sha256="a" * 64,
            agent_path="agent.py",
        )
        self.assertFalse(runner.register_strike(runtime))
        self.assertFalse(runner.register_strike(runtime))
        self.assertTrue(runner.register_strike(runtime))
        self.assertTrue(runtime.disqualified)
        self.assertIsNotNone(runtime.fixed)
        self.assertEqual(runtime.strikes, 3)

    def test_repeated_runtime_startup_failures_become_three_strikes(self):
        class Player:
            def __init__(self, player_id):
                self.player_id = player_id
                self.cash = 1_000
                self.position = 0
                self.in_jail = False
                self.jail_turns = 0
                self.gooj_card = False
                self.bankrupt = False
                self.properties = []

            def net_worth(self):
                return float(self.cash)

        class Env:
            def __init__(self):
                self.done = False
                self.round = 0
                self.phase = "pre_roll"
                self.has_rolled = False
                self.last_dice = (1, 1)
                self.houses_available = 32
                self.hotels_available = 12
                self.players = [Player(index) for index in range(4)]
                self.properties = {}

            def whose_turn(self):
                return 0

            def active_player_id(self):
                return 0

            def get_allowed_actions(self, _player_id):
                return [1, 2]

        class Game:
            def __init__(self):
                self.env = Env()
                self.steps = 0

            def step(self, _action):
                self.steps += 1
                self.env.round = self.steps
                self.env.done = self.steps == 3
                return None, 0.0, self.env.done, {"phase": "pre_roll"}

        submitted = runner.SeatRuntime(0, "A", "entry-a", None, "a" * 64, "agent.py")

        def cannot_start():
            raise runner.AgentCallError("startup", "cannot import agent")

        submitted.factory = cannot_start
        seats = [submitted]
        for seat in range(1, 4):
            bot = runner.SeatRuntime(seat, f"Bot {seat}", None, "hoarder", None, None)
            bot.fixed = runner.TheHoarder(seat)
            seats.append(bot)
        job = {
            "id": "game-id",
            "attempt_no": 1,
            "lease_id": "lease-id",
            "seed": 42,
            "ruleset_version": runner.RULESET_VERSION,
            "schema_version": runner.SCHEMA_VERSION,
            "max_rounds": runner.MAX_ROUNDS,
            "action_ceiling": runner.ACTION_CEILING,
            "play_limit_us": runner.PLAY_LIMIT_US,
            "seats": [],
        }
        with (
            mock.patch.object(runner, "prepare_seats", return_value=seats),
            mock.patch.object(runner.SharedGame, "new", return_value=Game()),
            mock.patch.object(runner, "decision_state", return_value={}),
        ):
            result = runner.drive_game(job, object(), upload_events=False)
        self.assertEqual(result["seats"][0]["strikes"], 3)
        self.assertEqual(result["seats"][0]["decision_count"], 3)
        self.assertTrue(result["seats"][0]["disqualified"])

    def test_timing_aggregation_and_timeout_weight(self):
        runtime = runner.SeatRuntime(0, "A", "e", None, "a" * 64, "agent.py")
        for duration in [1_000, 2_000_000, 4_000]:
            runtime.record_decision(duration)
        self.assertEqual(runtime.decision_count, 3)
        self.assertEqual(runtime.decision_min_us, 1_000)
        self.assertEqual(runtime.decision_max_us, 2_000_000)
        self.assertEqual(runtime.decision_total_us, 2_005_000)


class MatchRuleTests(unittest.TestCase):
    class Player:
        def __init__(self, worth):
            self._worth = worth

        def net_worth(self):
            return self._worth

    class Env:
        def __init__(self, worths):
            self.players = [MatchRuleTests.Player(value) for value in worths]

    def seats(self):
        return [
            runner.SeatRuntime(0, "bot", None, "hoarder", None, None),
            runner.SeatRuntime(1, "A", "a", None, "1" * 64, "agent.py"),
            runner.SeatRuntime(2, "B", "b", None, "2" * 64, "agent.py"),
            runner.SeatRuntime(3, "C", "c", None, "3" * 64, "agent.py"),
        ]

    def job(
        self,
        *,
        action_ceiling=runner.ACTION_CEILING,
        play_limit_us=runner.PLAY_LIMIT_US,
    ):
        return {
            "id": "game-id",
            "attempt_no": 1,
            "lease_id": "lease-id",
            "seed": 42,
            "ruleset_version": runner.RULESET_VERSION,
            "schema_version": runner.SCHEMA_VERSION,
            "max_rounds": runner.MAX_ROUNDS,
            "action_ceiling": action_ceiling,
            "play_limit_us": play_limit_us,
            "seats": [],
        }

    def submitted_seats(self):
        return [
            runner.SeatRuntime(
                seat=index,
                label=f"Team {index}",
                entry_id=f"entry-{index}",
                bot_key=None,
                artifact_sha256=str(index) * 64,
                agent_path="agent.py",
            )
            for index in range(4)
        ]

    def test_normal_winner_is_highest_net_worth_submitted_team(self):
        winner = runner.promoted_winner(
            self.submitted_seats(), self.Env([100, 900, 400, 700])
        )
        self.assertEqual(winner.entry_id, "entry-1")

    def test_bot_is_promoted_out_of_winner_selection(self):
        winner = runner.promoted_winner(self.seats(), self.Env([9_999, 500, 700, 600]))
        self.assertEqual(winner.entry_id, "b")

    def test_disqualified_team_cannot_win(self):
        seats = self.seats()
        seats[2].disqualified = True
        winner = runner.promoted_winner(seats, self.Env([9_999, 500, 7_000, 600]))
        self.assertEqual(winner.entry_id, "c")

    def test_no_submitted_team_means_no_winner(self):
        bots = [
            runner.SeatRuntime(index, "bot", None, "hoarder", None, None)
            for index in range(4)
        ]
        self.assertIsNone(runner.promoted_winner(bots, self.Env([100, 200, 300, 400])))

    def test_ten_minute_rule_selects_highest_average_only(self):
        seats = self.seats()
        seats[1].record_decision(500_000)
        seats[2].record_decision(2_000_000)
        seats[3].record_decision(1_000_000)
        self.assertEqual(runner.slowest_eligible(seats).entry_id, "b")

    def test_ten_minute_branch_disqualifies_slowest_then_promotes_winner(self):
        seats = self.submitted_seats()
        for seat, duration in zip(seats, [100_000, 900_000, 200_000, 300_000]):
            seat.record_decision(duration)
        with (
            mock.patch.object(runner, "PLAY_LIMIT_US", 0),
            mock.patch.object(runner, "prepare_seats", return_value=seats),
        ):
            result = runner.drive_game(
                self.job(play_limit_us=0), object(), upload_events=False
            )
        self.assertEqual(result["end_reason"], "ten_minute")
        self.assertTrue(result["seats"][1]["disqualified"])
        self.assertNotEqual(result["winner_entry_id"], "entry-1")

    def test_decision_that_crosses_time_limit_is_applied_and_timed(self):
        class Player:
            def __init__(self, player_id):
                self.player_id = player_id
                self.cash = 100
                self.position = 0
                self.in_jail = False
                self.jail_turns = 0
                self.gooj_card = False
                self.bankrupt = False
                self.properties = []

            def net_worth(self):
                return float(self.cash)

        class Env:
            done = False
            round = 1
            phase = "pre_roll"
            has_rolled = False
            last_dice = None
            houses_available = 32
            hotels_available = 12
            properties = {}
            players = [Player(index) for index in range(4)]

            @staticmethod
            def whose_turn():
                return 0

            @staticmethod
            def active_player_id():
                return 0

            @staticmethod
            def get_allowed_actions(_player_id):
                return [1, 2]

        class Game:
            def __init__(self):
                self.env = Env()
                self.steps = 0

            def step(self, _action):
                self.steps += 1
                self.env.done = True
                return None, 0.0, True, {"phase": "pre_roll"}

        seats = self.submitted_seats()
        process = mock.Mock()
        process.choose_action.return_value = 1
        process.close.return_value = ""
        seats[0].process = process
        for seat in seats[1:]:
            seat.entry_id = None
            seat.bot_key = "hoarder"
            seat.fixed = runner.TheHoarder(seat.seat)
        clock = iter([0, 30_000, 60_000, 90_000, 120_000, 150_000])
        with (
            mock.patch.object(runner, "PLAY_LIMIT_US", 100),
            mock.patch.object(runner, "prepare_seats", return_value=seats),
            mock.patch.object(runner.SharedGame, "new", return_value=Game()),
            mock.patch.object(runner, "decision_state", return_value={}),
            mock.patch.object(runner, "build_snapshot", return_value={"done": True}),
            mock.patch.object(runner.time, "monotonic_ns", side_effect=clock),
        ):
            result = runner.drive_game(
                self.job(play_limit_us=100), object(), upload_events=False
            )
        self.assertEqual(result["end_reason"], "ten_minute")
        self.assertEqual(result["action_count"], 1)
        self.assertEqual(result["seats"][0]["decision_count"], 1)
        self.assertEqual(result["seats"][0]["decision_total_us"], 30)
        self.assertTrue(result["seats"][0]["disqualified"])
        self.assertEqual(result["duration_us"], 150)
        self.assertTrue(result["seats"][0]["final_snapshot"]["done"])

    def test_action_ceiling_does_not_add_timing_disqualification(self):
        seats = self.submitted_seats()
        with (
            mock.patch.object(runner, "ACTION_CEILING", 1),
            mock.patch.object(runner, "prepare_seats", return_value=seats),
        ):
            result = runner.drive_game(
                self.job(action_ceiling=1), object(), upload_events=False
            )
        self.assertEqual(result["end_reason"], "action_ceiling")
        self.assertEqual(result["action_count"], 1)
        self.assertFalse(any(seat["disqualified"] for seat in result["seats"]))

    def test_no_eligible_winner_preserves_termination_reason(self):
        seats = self.submitted_seats()
        for seat in seats:
            seat.replace_with_bot()
        with (
            mock.patch.object(runner, "PLAY_LIMIT_US", 0),
            mock.patch.object(runner, "prepare_seats", return_value=seats),
        ):
            result = runner.drive_game(
                self.job(play_limit_us=0), object(), upload_events=False
            )
        self.assertEqual(result["end_reason"], "ten_minute")
        self.assertIsNone(result["winner_seat"])
        self.assertIsNone(result["winner_entry_id"])

    def test_decision_seed_and_fallback_are_deterministic(self):
        first = runner.splitmix64(1234 ^ (77 << 8) ^ 2)
        second = runner.splitmix64(1234 ^ (77 << 8) ^ 2)
        self.assertEqual(first, second)
        self.assertEqual(
            runner.deterministic_fallback([9, 2, 7], first),
            runner.deterministic_fallback([9, 2, 7], second),
        )

    def test_shared_game_seed_does_not_consume_ambient_rng(self):
        import random

        random.seed(99)
        expected = random.random()
        random.seed(99)
        left = SharedGame.new(42, 2)
        self.assertEqual(random.random(), expected)
        right = SharedGame.new(42, 2)
        for _ in range(12):
            left_action = left.env.get_allowed_actions(left.env.whose_turn())[0]
            right_action = right.env.get_allowed_actions(right.env.whose_turn())[0]
            self.assertEqual(left_action, right_action)
            left.step(left_action)
            right.step(right_action)
        self.assertEqual(
            runner.build_snapshot(left.env), runner.build_snapshot(right.env)
        )


class ValidationTests(unittest.TestCase):
    def test_default_branch_commit_is_pinned(self):
        branch, sha = controller.parse_default_head(
            "ref: refs/heads/main\tHEAD\n" + "a" * 40 + "\tHEAD\n"
        )
        self.assertEqual(branch, "refs/heads/main")
        self.assertEqual(sha, "a" * 40)

    def test_requirements_are_wheel_only_pypi_and_bounded(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "requirements.txt"
            path.write_text("numpy==2.3.1\nonnxruntime>=1.20,<2\n")
            self.assertEqual(len(controller.validate_requirements(path)), 2)
            for unsafe in [
                "evil @ https://example.com/evil.whl\n",
                "-r other.txt\n",
                "../local-package\n",
                "\n".join(f"package-{index}" for index in range(33)),
            ]:
                path.write_text(unsafe)
                with self.assertRaises(controller.ValidationFailed):
                    controller.validate_requirements(path)

    def test_lfs_size_counts_resolved_object_before_download(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "weights.bin"
            path.write_text(
                "version https://git-lfs.github.com/spec/v1\n"
                "oid sha256:" + "a" * 64 + "\nsize 123456\n"
            )
            self.assertEqual(controller.lfs_pointer_size(path), 123456)
            self.assertEqual(
                controller.resolved_tree_size(Path(temporary)), (123456, True)
            )
            path.write_text(
                "version https://git-lfs.github.com/spec/v1\n"
                "oid sha256:" + "a" * 64 + "\nsize " + str(251 * 1024**2) + "\n"
            )
            with self.assertRaisesRegex(controller.ValidationFailed, "250 MiB"):
                controller.resolved_tree_size(Path(temporary))

    def test_checkout_and_archive_reject_links_and_traversal(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "agent.py").write_text("pass\n")
            os.symlink(root / "agent.py", root / "linked.py")
            with self.assertRaisesRegex(controller.ValidationFailed, "symlink"):
                controller.resolved_tree_size(root)
        traversal = tarfile.TarInfo("../escape")
        traversal.size = 1
        symlink = tarfile.TarInfo("linked")
        symlink.type = tarfile.SYMTYPE
        self.assertFalse(runner._safe_archive_member(traversal))
        self.assertFalse(runner._safe_archive_member(symlink))

    def test_archive_is_hash_stable_and_has_no_unsafe_members(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            root.mkdir()
            (root / "metadata.json").write_text(json.dumps({"artifact_schema": 1}))
            (root / "submission").mkdir()
            (root / "submission" / "agent.py").write_text(
                "def choose_action(*x): return 0\n"
            )
            first, second = Path(temporary) / "a.tar.gz", Path(temporary) / "b.tar.gz"
            controller.normalized_archive(root, first)
            controller.normalized_archive(root, second)
            self.assertEqual(
                controller.sha256_file(first), controller.sha256_file(second)
            )
            with tarfile.open(first, "r:gz") as archive:
                self.assertTrue(
                    all(
                        runner._safe_archive_member(member)
                        for member in archive.getmembers()
                    )
                )

    def test_fresh_artifact_cache_is_marked_ready_and_reused(self):
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            staging = temporary / "staging"
            staging.mkdir()
            (staging / "metadata.json").write_text(
                json.dumps({"artifact_schema": 1})
            )
            (staging / "submission").mkdir()
            (staging / "submission" / "agent.py").write_text(
                "def choose_action(state, player_id, allowed_actions):\n"
                "    return allowed_actions[0]\n"
            )
            (staging / "wheelhouse").mkdir()
            (staging / "requirements.lock").write_text("")
            (staging / "agent_runner.py").write_text("pass\n")
            source = temporary / "source.tar.gz"
            controller.normalized_archive(staging, source)
            sha256 = controller.sha256_file(source)

            class Client:
                calls = 0

                def download(self, _path, destination):
                    self.calls += 1
                    destination.write_bytes(source.read_bytes())

            client = Client()
            with (
                mock.patch.object(runner, "CACHE_DIR", temporary / "cache"),
                mock.patch.object(runner, "BACKEND", "docker"),
            ):
                first = runner.cache_artifact(client, sha256)
                second = runner.cache_artifact(client, sha256)
            self.assertEqual(first, second)
            self.assertTrue((first / ".ready").is_file())
            self.assertEqual(client.calls, 1)

    def test_archive_authorization_path_accepts_only_lowercase_sha(self):
        self.assertEqual(len("a" * 64), 64)
        for invalid in ["a" * 63, "A" * 64, "../" + "a" * 64]:
            self.assertFalse(
                len(invalid) == 64 and all(c in "0123456789abcdef" for c in invalid)
            )


class FleetTests(unittest.TestCase):
    def test_last_crashed_worker_rearms_fleet(self):
        fleet = object.__new__(controller.Fleet)
        process = mock.Mock()
        process.poll.return_value = 1
        process.returncode = 1
        fleet.processes = {"ai-monopoly-1": process}
        fleet.hosts = {}
        fleet.allocated = {"ai-monopoly-1"}
        fleet.attempted = True
        with (
            mock.patch.object(controller, "stop_session"),
            mock.patch.object(controller, "report_fleet"),
        ):
            fleet.reap()
        self.assertFalse(fleet.attempted)
        self.assertFalse(fleet.processes)
        self.assertFalse(fleet.allocated)

    def test_shutdown_kills_a_process_that_ignores_terminate(self):
        process = mock.Mock()
        process.poll.return_value = None
        process.wait.side_effect = [
            controller.subprocess.TimeoutExpired("worker", 0.01),
            0,
        ]

        controller.terminate_process(process, timeout=0.01)

        process.terminate.assert_called_once_with()
        process.kill.assert_called_once_with()
        self.assertEqual(process.wait.call_count, 2)

    def test_preflight_payload_exposes_all_qualification_fields(self):
        payload = controller.parse_preflight(
            'noise\nEXPOSURE_PREFLIGHT={"ram_bytes":34359738368,"effective_vcpus":8,"machine_shape":"n2-highmem","python":"3.12"}\n'
        )
        self.assertEqual(payload["ram_bytes"], 32 * 1024**3)
        self.assertEqual(payload["effective_vcpus"], 8)
        self.assertEqual(payload["machine_shape"], "n2-highmem")

    def test_failed_session_listing_is_not_mistaken_for_clean_shutdown(self):
        failed = controller.subprocess.CompletedProcess(
            ["colab", "sessions"], 1, stdout="", stderr="authentication failed"
        )
        with (
            mock.patch.object(controller, "colab", return_value=failed),
            self.assertRaisesRegex(RuntimeError, "authentication failed"),
        ):
            controller.active_named_sessions()

    def test_allocation_timeout_is_reported_and_not_retried(self):
        stopped = controller.subprocess.CompletedProcess(
            ["colab", "stop"], 0, stdout="", stderr=""
        )
        with (
            mock.patch.object(
                controller,
                "colab",
                side_effect=[
                    controller.subprocess.TimeoutExpired("colab new", 300),
                    stopped,
                ],
            ) as colab,
            mock.patch.object(controller, "report_fleet") as report,
        ):
            self.assertIsNone(controller.allocate_qualified("ai-monopoly-1"))
        self.assertEqual(colab.call_count, 2)
        self.assertEqual(colab.call_args_list[0].args[0], ["new", "-s", "ai-monopoly-1"])
        report.assert_called_once()
        self.assertEqual(report.call_args.args[1], "error")


if __name__ == "__main__":
    unittest.main()
