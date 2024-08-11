#include <nlohmann/json.hpp>
#include <httplib.h>

using json = nlohmann::json;
using namespace httplib;
using namespace std;

int main() {
  Server server;

  server.Get(".*", [](const Request& req, Response& res) {
    json response;
    response["path"] = req.path;
    res.set_content(response.dump(), "application/json");
    cout << "GET " << req.path << "\n" << response.dump() << "\n" << endl;
  });

  cout << "Server started at http://localhost:7468" << endl;
  server.listen("localhost", 7468);

  return 0;
}
