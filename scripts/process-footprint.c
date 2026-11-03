/* Query the macOS kernel's current and lifetime peak physical footprint. */
#include <errno.h>
#include <libproc.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: process-footprint PID\n");
        return 2;
    }
    char *end = NULL;
    errno = 0;
    long pid = strtol(argv[1], &end, 10);
    if (errno || !end || *end || pid <= 0 || pid > INT32_MAX) {
        fprintf(stderr, "invalid PID\n");
        return 2;
    }
    struct rusage_info_v4 usage = {0};
    if (proc_pid_rusage((int)pid, RUSAGE_INFO_V4, (rusage_info_t *)&usage) != 0) {
        perror("proc_pid_rusage");
        return 1;
    }
    printf("{\"pid\":%ld,\"start_abstime\":%llu,\"physical_bytes\":%llu,\"lifetime_peak_physical_bytes\":%llu}\n",
           pid, (unsigned long long)usage.ri_proc_start_abstime,
           (unsigned long long)usage.ri_phys_footprint,
           (unsigned long long)usage.ri_lifetime_max_phys_footprint);
    return 0;
}
