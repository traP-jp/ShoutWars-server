#include "room.hpp"

#include <utility>

using namespace std;

boost::uuids::time_generator_v7 room_t::gen_id{};

// room user

room_t::user_t::user_t(string name) : name(move(name)) {}

// room

room_t::room_t(string version, const user_t &owner, const int size)
  : version(move(version)), size(size), users{ owner } {}

// room list

room_list_t::room_list_t(logger log_info, logger log_error, const int limit)
  : log_info(move(log_info)), log_error(move(log_error)), limit(limit) {}

shared_ptr<room_t> room_list_t::create(const string &version, const room_t::user_t &owner, const int size) {
  lock_guard lock(rooms_mutex);
  if (rooms.size() >= limit) throw pair<int, string>(503, "Room limit reached.");
  auto room = make_shared<room_t>(version, owner, size);
  rooms[room->id] = room;
  return room;
}
