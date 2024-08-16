#include "room.hpp"

#include <nlohmann/json.hpp>
#include <httplib.h>
#include <boost/uuid/uuid_io.hpp>
#include <boost/uuid/uuid_generators.hpp>
#include <boost/uuid/uuid.hpp>
#include <iostream>
#include <format>
#include <string>
#include <functional>
#include <exception>
#include <utility>
#include <cstdlib>

#include "session.hpp"

using namespace std;
using namespace boost::uuids;

using Server = httplib::Server;
using Request = httplib::Request;
using Response = httplib::Response;
using json = nlohmann::json;

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

constexpr string_generator gen_uuid_from_string{};

template<> struct nlohmann::adl_serializer<uuid> {
  static void to_json(json &j, const uuid &uuid) { j = to_string(uuid); }

  static void from_json(const json &j, uuid &uuid) {
    try {
      uuid = gen_uuid_from_string(j.get<string>());
    } catch (const runtime_error &err) {
      throw pair(400, format("Invalid UUID: {}", err.what()));
    }
  }
};

auto gen_auth_handler(const function<json(const json &)> &handle_json) {
  return [=](const Request &req, Response &res) {
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
  session_list_t session_list(log_stdout, log_stderr);
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
    [&](const json &req) -> json { throw pair<int, string>(404, format("Invalid API version. Use {}.", api_path)); }
  );
  server.Get(invalid_ver_pattern, invalid_ver_handler);
  server.Post(invalid_ver_pattern, invalid_ver_handler);

  server.Post(
    api_path + "/room/create"s,
    gen_auth_handler(
      [&](const json &req) -> json {
        const string version = req.at("version");
        const room_t::user_t owner(req.at("user").at("name"));
        const size_t size = req.at("size");
        const auto room = room_list.create(version, owner, size);
        const auto session = session_list.create(room->id, owner.id);
        return { { "session_id", session.id }, { "user_id", owner.id }, { "id", room->id } };
      }
    )
  );

  server.Get(
    api_path + "/status"s,
    gen_auth_handler(
      [&](const json &req) -> json {
        return { { "room_count", room_list.count() }, { "room_limit", room_list.limit } };
      }
    )
  );

  room_list.start_cleaner(1s, 5s);

  log_stdout(format("Server started at http://localhost:{}", port));
  if (!password.empty()) log_stdout(format("Password: {}", password));
  server.listen("0.0.0.0", port);

  return 0;
}
