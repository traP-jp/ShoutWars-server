cmake_minimum_required(VERSION 3.21)
project(ShoutWars_server)

set(CMAKE_CXX_STANDARD 23)

include(FetchContent)

FetchContent_Declare(json URL https://github.com/nlohmann/json/releases/download/v3.11.3/json.tar.xz)
FetchContent_MakeAvailable(json)

FetchContent_Declare(httplib GIT_REPOSITORY https://github.com/yhirose/cpp-httplib.git GIT_TAG v0.16.2)
FetchContent_MakeAvailable(httplib)

add_executable(ShoutWars_server main.cpp)

target_link_libraries(ShoutWars_server PRIVATE nlohmann_json::nlohmann_json httplib::httplib)
