#include <nlohmann/json.hpp>
#include <httplib.h>
#include <boost/uuid/time_generator_v7.hpp>
#include <boost/uuid/uuid_io.hpp>
#include <iostream>
#include <format>
#include <string>
#include <cstdlib>

using namespace nlohmann;
using namespace httplib;
using namespace boost::uuids;
using namespace std;

string getenv_or(const string &key, const string &default_value) {
  const char *value = getenv(key.c_str());
  return value ? value : default_value;
}

constexpr int api_ver = 0;
const string api_path(format("/v{}", api_ver));

const int port = stoi(getenv_or("PORT", "7468"));
const string password = getenv_or("PASSWORD", "");
const int room_limit = stoi(getenv_or("ROOM_LIMIT", "100"));

void responseMsgpack(Response &res, const json &data) {
  const auto msgpack = json::to_msgpack(data);
  res.set_content(string(msgpack.begin(), msgpack.end()), "application/msgpack");
}

bool check_password(const Request &req, Response &res) {
  if (password.empty() || req.get_header_value("Authorization") == "Bearer "s + password) return false;
  res.status = 404;
  return true;
}

void fallback_version(Server &server) {
  const string pattern = format(R"((?!{}/).*)", api_path);
  const Server::Handler handler = [&](const Request &req, Response &res) {
    if (check_password(req, res)) return;
    res.status = 404;
    responseMsgpack(res, {
      { "error", format("Invalid API version. Use {}.", api_path) },
    });
  };
  server.Get(pattern, handler);
  server.Post(pattern, handler);
}

int main() {
  time_generator_v7 id_gen;
  Server server;

  server.Get(api_path + "/status"s, [&](const Request &req, Response &res) {
    if (check_password(req, res)) return;
    responseMsgpack(res, {
      { "room_count", 0 },
      { "room_limit", room_limit },
    });
  });

  fallback_version(server);

  cout << format("Server started at http://localhost:{}", port) << endl;
  if (!password.empty()) cout << format("Password: {}", password) << endl;
  server.listen("0.0.0.0", port);

  return 0;
}
