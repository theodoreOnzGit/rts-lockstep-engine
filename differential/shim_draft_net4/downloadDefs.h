/* Stand-in for WWDownload/downloadDefs.h (#215, group net4): the file is not in the upstream tree (the vendored WWDownload folder is empty), so the numeric
** values of DOWNLOADEVENT_* and DOWNLOADSTATUS_* are not known. These are distinct values chosen here; the test maps the draft's enums onto them, so
** what is compared is the switch's mapping of each event to its text label, not the numbers. Never upstream code. */
#ifndef NET4_DOWNLOADDEFS_H
#define NET4_DOWNLOADDEFS_H
#define DOWNLOADEVENT_NOSUCHSERVER 1
#define DOWNLOADEVENT_COULDNOTCONNECT 2
#define DOWNLOADEVENT_LOGINFAILED 3
#define DOWNLOADEVENT_NOSUCHFILE 4
#define DOWNLOADEVENT_LOCALFILEOPENFAILED 5
#define DOWNLOADEVENT_TCPERROR 6
#define DOWNLOADEVENT_DISCONNECTERROR 7
#define DOWNLOADEVENT_RESUME 1
#define DOWNLOADEVENT_DONOTRESUME 0
#define DOWNLOADSTATUS_CONNECTING 1
#define DOWNLOADSTATUS_LOGGINGIN 2
#define DOWNLOADSTATUS_FINDINGFILE 3
#define DOWNLOADSTATUS_QUERYINGRESUME 4
#define DOWNLOADSTATUS_DOWNLOADING 5
#define DOWNLOADSTATUS_DISCONNECTING 6
#define DOWNLOADSTATUS_FINISHING 7
#define DOWNLOADSTATUS_DONE 8
#endif
