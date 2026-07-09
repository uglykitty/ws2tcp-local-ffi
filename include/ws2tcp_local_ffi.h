#ifndef WS2TCP_LOCAL_FFI_H
#define WS2TCP_LOCAL_FFI_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct Ws2TcpHandle Ws2TcpHandle;
typedef void (*Ws2TcpLogCallback)(const char *message, void *user_data);

typedef enum Ws2TcpStatus {
  WS2TCP_STATUS_STOPPED = 0,
  WS2TCP_STATUS_RUNNING = 1,
} Ws2TcpStatus;

enum {
  WS2TCP_OK = 0,
  WS2TCP_ERROR_NULL_HANDLE = 1,
  WS2TCP_ERROR_INVALID_ARGUMENT = 2,
  WS2TCP_ERROR_ALREADY_RUNNING = 3,
  WS2TCP_ERROR_RUNTIME = 4,
};

Ws2TcpHandle *ws2tcp_handle_new(void);
void ws2tcp_handle_free(Ws2TcpHandle *handle);

int ws2tcp_init_logging(const char *log_level);
int ws2tcp_set_log_callback(Ws2TcpLogCallback callback, void *user_data,
                            const char *log_level);

int ws2tcp_start(Ws2TcpHandle *handle, const char *config_json);
int ws2tcp_stop(Ws2TcpHandle *handle);
Ws2TcpStatus ws2tcp_status(Ws2TcpHandle *handle);
const char *ws2tcp_last_error(Ws2TcpHandle *handle);

#ifdef __cplusplus
}
#endif

#endif
