#include <nlohmann/json.hpp>
#include <httplib.h>
#include <boost/uuid/time_generator_v7.hpp>
#include <boost/uuid/uuid_io.hpp>
#include <iostream>

using namespace nlohmann;
using namespace httplib;
using namespace boost::uuids;
using namespace std;

int main() {
  time_generator_v7 id_gen;
  Server server;

  server.Get(".*", [&](const Request& req, Response& res) {
    json response;
    response["path"] = req.path;
    response["id"] = to_string(id_gen());
    res.set_content(response.dump(), "application/json");
    cout << "GET " << req.path << "\n" << response.dump() << "\n" << endl;
  });

  cout << "Server started at http://localhost:7468" << endl;
  server.listen("0.0.0.0", 7468);

  return 0;
}
