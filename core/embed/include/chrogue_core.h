/*
  The C interface of the Chrogue core. A client that loads the core into its own process uses it in place of the
  socket of chrogue-core. The requests and the responses are the lines of core/PROTOCOL.md, with no newline.

  All text is UTF-8 with a 0 at its end. One thread at a time can use a core.
*/
#ifndef CHROGUE_CORE_H
#define CHROGUE_CORE_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ChrogueCore ChrogueCore;

/*
  Opens a core on the title screen. `options` is a JSON object, and each field is optional:
    "save_dir": the folder of the saved data. With no folder, the core keeps the saved data in memory.
    "seed": the seed of the random numbers, as a number or as a string of digits. The default comes from the clock.
    "debug": true to accept the debug commands.
  Returns NULL if the core cannot open. chrogue_open_error then gives the reason.
*/
ChrogueCore *chrogue_open(const char *options);

/* The reason of the last chrogue_open of this thread that returned NULL. The text stays until the next chrogue_open. */
const char *chrogue_open_error(void);

/*
  Runs one request and returns its response. The response stays until the next chrogue_command or chrogue_close of
  this core. The function does not fail: a bad request gets an error response. A NULL core, and a fault inside the
  core, get an error response with the code "internal" and no "view".
*/
const char *chrogue_command(ChrogueCore *core, const char *request);

/* Closes the core and releases the lock of its save folder. NULL is permitted. */
void chrogue_close(ChrogueCore *core);

#ifdef __cplusplus
}
#endif

#endif
