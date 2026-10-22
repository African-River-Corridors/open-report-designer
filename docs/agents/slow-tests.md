# Slow tests

The pull-request gate (`.config/nextest.toml`, profile `pr`) leaves out the tests below. The nightly workflow
runs every test. A test moves here only by measured time (the nextest JUnit report or its timing output), with
one reason. No test is deleted.

| Test (binary, name) | Measured time | Reason |
|---|---|---|
| *none yet* | | |
