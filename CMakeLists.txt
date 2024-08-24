cmake_minimum_required(VERSION 3.21)
project(ShoutWars_server)

set(CMAKE_CXX_STANDARD 20)

include(FetchContent)

FetchContent_Declare(json URL https://github.com/nlohmann/json/releases/download/v3.11.3/json.tar.xz)
FetchContent_MakeAvailable(json)

FetchContent_Declare(httplib GIT_REPOSITORY https://github.com/yhirose/cpp-httplib.git GIT_TAG v0.16.2)
FetchContent_MakeAvailable(httplib)

set(BOOST_INCLUDE_LIBRARIES uuid)
set(BOOST_ENABLE_CMAKE ON)
FetchContent_Declare(Boost URL https://github.com/boostorg/boost/releases/download/boost-1.86.0.beta1/boost-1.86.0.beta1-cmake.tar.xz)
FetchContent_MakeAvailable(Boost)

add_executable(ShoutWars_server main.cpp session.cpp room_list.cpp room.cpp sync_record.cpp)

target_link_libraries(ShoutWars_server PRIVATE nlohmann_json::nlohmann_json httplib::httplib Boost::uuid)
