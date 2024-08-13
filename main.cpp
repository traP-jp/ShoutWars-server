#include "room.hpp"

#include <nlohmann/json.hpp>
#include <httplib.h>
#include <boost/uuid/uuid_io.hpp>
#include <iostream>
#include <format>
#include <string>
#include <functional>
#include <exception>
#include <cstdlib>

using namespace std;

using json = nlohmann::json;
using Server = httplib::Server;
using Request = httplib::Request;
using Response = httplib::Response;

string getenv_or(const string &key, const string &default_value) {
  const char *value = getenv(key.c_str());
  return value ? value : default_value;
}

constexpr int api_ver = 0;
const string api_path(format("/v{}", api_ver));

const int port = stoi(getenv_or("PORT", "7468"));
const string password = getenv_or("PASSWORD", "");
const int room_limit = stoi(getenv_or("ROOM_LIMIT", "100"));

void log_stdout(const string &msg) { cout << msg << endl; }
void log_stderr(const string &msg) { cerr << msg << endl; }

auto gen_auth_handler(const function<json(const json &)> &handle_json) {
  return [&](const Request &req, Response &res) {
    if (!password.empty() && req.get_header_value("Authorization") != "Bearer "s + password) {
      res.status = 404;
      return;
    }
    try {
      const auto msgpack = json::to_msgpack(
        handle_json(req.body.empty() ? json(nullptr) : json::from_msgpack(req.body))
      );
      res.set_content(string(msgpack.begin(), msgpack.end()), "application/msgpack");
    } catch (const json::exception &err) {
      throw pair<int, string>(400, err.what());
    }
  };
}

int main() {
  room_list_t room_list(log_stdout, log_stderr, room_limit);

  Server server;

  server.set_exception_handler(
    [&](const Request &req, Response &res, const exception_ptr &ep) {
      try {
        rethrow_exception(ep);
      } catch (const pair<int, string> &err) {
        res.status = err.first;
        const auto msgpack = json::to_msgpack(json{ { "error", err.second } });
        res.set_content(string(msgpack.begin(), msgpack.end()), "application/msgpack");
      } catch (const exception &err) {
        res.status = 500;
        log_stderr(format("Internal server error: {}\n  when {} {}", err.what(), req.method, req.path));
      } catch (...) {
        res.status = 500;
        log_stderr(format("Unknown error\n  when {} {}", req.method, req.path));
      }
    }
  );

  const string invalid_ver_pattern = format(R"((?!{}/).*)", api_path);
  const Server::Handler invalid_ver_handler = gen_auth_handler(
    [&](const json &req)-> json {
      return { { "error", format("Invalid API version. Use {}.", api_path) } };
    }
  );
  server.Get(invalid_ver_pattern, invalid_ver_handler);
  server.Post(invalid_ver_pattern, invalid_ver_handler);

  server.Get(
    api_path + "/status"s,
    gen_auth_handler(
      [&](const json &req) -> json {
        return { { "room_count", room_list.rooms.size() }, { "room_limit", room_list.limit } };
      }
    )
  );

  log_stdout(format("Server started at http://localhost:{}", port));
  if (!password.empty()) log_stdout(format("Password: {}", password));
  server.listen("0.0.0.0", port);

  return 0;
}
