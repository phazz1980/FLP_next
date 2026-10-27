/*
 * FLProg_Network — flprogWebServer.h
 *
 * Legacy FLProg generated-code compatibility facade.
 *
 * FLProg 9.x generated web projects include this historical header and use
 * the FLProgWebServer type. The new transport-independent implementation is
 * FLProgNetworkHttpServer.
 *
 * This alias intentionally keeps both names the SAME C++ type:
 *  - legacy callback signatures remain compatible;
 *  - no wrapper object / extra RAM is introduced;
 *  - addHandler(), sendDefault200Page(), sendDefault404Page(),
 *    argument/header access and print()/println() use the existing
 *    FLProgNetworkHttpServer implementation;
 *  - generated FLProg code does not need manual edits.
 *
 * WS01 is deliberately minimal. If a generated project uses an older
 * constructor or another legacy member not yet covered by
 * FLProgNetworkHttpServer, the next compile error becomes the next
 * compatibility gate.
 * Расширенный контракт сопровождения:
 * - Это публичная или межмодульная декларация: комментарии описывают владельца состояния и допустимый порядок вызовов.
 * - Конфигурация применяется до начала работы; runtime-методы обслуживаются из общего network.pool().
 * - При изменении API учитывать legacy generated-код и frozen API level 10006.
 * Extended maintenance contract:
 * - This declaration defines a public or cross-module boundary; document state ownership and call order here.
 * - Apply configuration before start; service runtime through the shared network.pool() lifecycle.
 * - Any API change must account for legacy generated code and frozen API level 10006.
 */
#pragma once

#include "service/flprogNetworkHttpServer.h"

using FLProgWebServer = FLProgNetworkHttpServer;
using FLProgWebServerCallback = FLProgNetworkHttpServerCallback;

// WS02 note:
// FLProgNetworkHttpServer now also accepts the historical
// FLProgWebServer(FLProgNetworkInterface*, uint16_t) construction pattern
// and lazily starts from pool() after the interface is ready.

// WS03A: legacy generated callbacks stream print()/println() through the
// accepted TCP client. Native FLProgNetworkHttpServer keeps its bounded
// cooperative response buffer.

// WS03B: in legacy mode, modern browser optional headers that exceed the
// bounded ESP8266 header table are ignored instead of causing HTTP 400/431.
// Protocol-critical headers are still parsed. Native HTTP mode remains strict.
