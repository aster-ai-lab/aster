"""Validate the documented no-header session inventory after a waited primary process exits."""
def require_no_active_sessions(result):
    if type(result.returncode) is not int or result.returncode != 0:
        raise ValueError('nsys sessions list did not complete successfully')
    if not isinstance(result.stdout, str) or not isinstance(result.stderr, str):
        raise ValueError('nsys session inventory must be decoded text')
    if len(result.stdout) > 8192 or len(result.stderr) > 4096:
        raise ValueError('bounded nsys session inventory exceeded')
    if result.stderr.strip():
        raise ValueError('nsys session inventory returned an unclassified diagnostic')
    if result.stdout.strip():
        raise ValueError('nsys session remains after the waited primary process exited')
    return True
