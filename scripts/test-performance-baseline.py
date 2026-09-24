import importlib.util
import os
from pathlib import Path
import tempfile
import sys
import unittest

SPEC = importlib.util.spec_from_file_location("baseline", Path(__file__).with_name("performance-baseline.py"))
baseline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(baseline)


class BaselineTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform in ("win32", "darwin"), "native process counters")
    def test_native_sampler_reads_only_its_process(self):
        sampler = baseline.Sampler(os.getpid())
        try:
            sample = sampler.sample()
            self.assertGreater(sample["resident_bytes"], 0)
            self.assertGreaterEqual(sample["cpu_seconds"], 0)
            if sys.platform == "win32":
                self.assertGreater(sample["private_bytes"], 0)
            else:
                self.assertIsNone(sample["private_bytes"])
        finally:
            sampler.close()

    def summarize(self, contents):
        with tempfile.TemporaryDirectory() as root:
            paths = []
            for name, content in contents.items():
                path = Path(root) / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
                paths.append(path)
            return baseline.summarize_logs(paths)

    def test_nearest_rank_and_empty_samples(self):
        self.assertEqual(baseline.distribution(range(1, 31)), {"count": 30, "p50": 15, "p95": 29, "max": 30})
        self.assertIsNone(baseline.distribution([])["p95"])

    def test_restart_ids_do_not_collide(self):
        event = "search_latency id=1 stage=query_submitted elapsed_us=0\n"
        result = self.summarize({"run1/rotor.log": event + "search_latency id=1 stage=results_painted elapsed_us=5000",
                                 "run2/rotor.log": event})["requests"]["search"]
        self.assertEqual((result["started"], result["complete"], result["incomplete"]), (2, 1, 1))
        self.assertEqual(result["latency_ms"], [5.0])

    def test_rotated_logs_share_request_namespace(self):
        result = self.summarize({"run/rotor.previous.log": "search_latency id=1 stage=query_submitted elapsed_us=0",
                                 "run/rotor.log": "search_latency id=1 stage=results_painted elapsed_us=2000"})
        self.assertEqual(result["requests"]["search"]["complete"], 1)

    def test_capture_requires_all_displays(self):
        log = "\n".join([
            "capture_latency id=1 stage=capture_requested elapsed_us=0",
            "capture_latency id=1 stage=frames_ready monitors=2 elapsed_us=10",
            "capture_latency id=1 stage=mask_native_visible monitor=10 elapsed_us=3000",
            "capture_latency id=1 stage=mask_show_requested monitor=11 elapsed_us=4000",
        ])
        incomplete = self.summarize({"rotor.log": log})["requests"]["capture"]
        self.assertEqual(incomplete["incomplete"], 1)
        self.assertIsNone(incomplete["distribution_ms"]["p95"])
        result = self.summarize({"rotor.log": log + "\ncapture_latency id=1 stage=mask_native_visible monitor=11 elapsed_us=6000"})
        self.assertEqual(result["requests"]["capture"]["latency_ms"], [6.0])

    def test_duplicate_paints_and_ready_events_are_not_extra_samples(self):
        result = self.summarize({"rotor.log": "\n".join([
            "search_latency id=1 stage=query_submitted elapsed_us=0",
            "search_latency id=1 stage=results_painted elapsed_us=1000",
            "search_latency id=1 stage=results_painted elapsed_us=2000",
            "startup_latency id=0 stage=index_ready elapsed_us=5000",
            "startup_latency id=0 stage=index_ready elapsed_us=9000",
        ])})
        self.assertEqual(result["requests"]["search"]["latency_ms"], [1.0])
        self.assertEqual(result["stages_ms"]["startup.index_ready"]["p95"], 5.0)

    def test_dropped_logs_invalidate_timing(self):
        result = self.summarize({"rotor.log": "1 WARN [rotor] dropped 3 log lines\n1 ERROR [rotor] failed"})
        self.assertFalse(result["timing_data_valid"])
        self.assertEqual(result["dropped_log_lines"], 3)
        self.assertEqual(result["error_lines"], 1)
        self.assertEqual(result["requests"]["search"]["started"], 0)

    def test_rotation_order_does_not_change_first_ready(self):
        result = self.summarize({"run/rotor.log": "startup_latency id=0 stage=index_ready elapsed_us=9000",
                                 "run/rotor.previous.log": "startup_latency id=0 stage=index_ready elapsed_us=1000"})
        self.assertEqual(result["stages_ms"]["startup.index_ready"]["p50"], 1.0)

    def test_malformed_timing_is_rejected(self):
        with self.assertRaises(ValueError):
            self.summarize({"rotor.log": "search_latency id=1 stage=results_painted elapsed_us=-1"})

    def test_macos_cpu_time(self):
        self.assertEqual(baseline.ps_cpu_seconds("01:02.50"), 62.5)
        self.assertEqual(baseline.ps_cpu_seconds("1-02:03:04"), 93784)


if __name__ == "__main__":
    unittest.main()
