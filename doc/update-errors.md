# Update error codes

Update errors use a single status line. Find the displayed code in `rotor.log`
under the active profile directory to read the complete diagnostic. Use the
entry's timestamp and update revision to distinguish repeated failures.

| Code | Meaning |
| --- | --- |
| E-UPD-001 | Checking for updates failed or the check could not start |
| E-UPD-002 | Downloading or verifying the update failed, or the download could not start |
| E-UPD-003 | Starting installation failed or installation was requested before the update was ready |
| E-UPD-004 | The installer's product name or version differs from the selected update |
| E-UPD-005 | Opening the download directory failed |

Production uses `.rotor`; development uses `.rotor-dev`. An explicit data
directory overrides these defaults. Cancellation is not assigned an error code.
Codes describe the operation or validation failure; the log contains the
underlying cause. Signature and installer identity checks remain required.
