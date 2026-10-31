// peak-rss PROGRAM [ARG...]: runs PROGRAM and, once it ends, writes `peak-rss-kb N` to standard
// error, N being the most resident memory it held, in KB (its `ru_maxrss`); exits as it did.
//
// `scripts/benchmark.py` runs every program through it. The peak of a process includes the pages
// of the process it was forked from, up to its exec: Python's 20 MB, measured directly, were most
// of the peak of a small translated program (4 to 6 MB of its own). Forked from this small
// program, it starts from about 1 MB. PROGRAM is killed when this program is (as by the script's
// timeout), so that it does not run on holding the script's pipes open.
#include <signal.h>
#include <stdio.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: peak-rss PROGRAM [ARG...]\n");
        return 2;
    }
    pid_t parent = getpid();
    pid_t pid = fork();
    if (pid < 0) {
        perror("fork");
        return 2;
    }
    if (pid == 0) {
        if (prctl(PR_SET_PDEATHSIG, SIGKILL) < 0 || getppid() != parent) {
            _exit(127);
        }
        execvp(argv[1], argv + 1);
        perror(argv[1]);
        _exit(127);
    }
    int status;
    struct rusage usage;
    if (wait4(pid, &status, 0, &usage) < 0) {
        perror("wait4");
        return 2;
    }
    fprintf(stderr, "peak-rss-kb %ld\n", usage.ru_maxrss);
    return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
}
