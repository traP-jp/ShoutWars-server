#include "room_list.hpp"

#include "errors.hpp"
#include <format>
#include <utility>

using namespace std;
using namespace boost::uuids;

// room list

room_list_t::room_list_t(
  const size_t limit, const chrono::minutes lobby_lifetime, const chrono::minutes game_lifetime, logger log_error,
  logger log_info
)
  : log_error(move(log_error)), log_info(move(log_info)), lobby_lifetime(lobby_lifetime), game_lifetime(game_lifetime),
    limit(limit) {}

shared_ptr<room_t> room_list_t::create(const string &version, const room_t::user_t &owner, const size_t size) {
  lock_guard lock(rooms_mutex);
  if (rooms.size() >= limit) throw forbidden_error(format("Room limit reached. Max room count is {}.", limit));
  const auto room = make_shared<room_t>(version, owner, size, lobby_lifetime, game_lifetime, log_error, log_info);
  rooms[room->id] = room;
  log_info(
    format(
      "Room created: {} (version={}, owner_id={}, size={})",
      to_string(room->id),
      version,
      to_string(owner.id),
      size
    )
  );
  return room;
}

shared_ptr<room_t> room_list_t::get(const uuid id) const {
  shared_lock lock(rooms_mutex);
  try {
    return rooms.at(id);
  } catch (const out_of_range &) {
    throw not_found_error("Room not found.");
  }
}

bool room_list_t::exists(const uuid id) const {
  shared_lock lock(rooms_mutex);
  return rooms.contains(id);
}

bool room_list_t::remove(const uuid id) {
  lock_guard lock(rooms_mutex);
  if (rooms.erase(id) > 0) {
    log_info(format("Room removed: {}", to_string(id)));
    return true;
  }
  return false;
}

size_t room_list_t::count() const {
  shared_lock lock(rooms_mutex);
  return rooms.size();
}

vector<shared_ptr<room_t>> room_list_t::get_all() const {
  shared_lock lock(rooms_mutex);
  const auto view = rooms | views::transform([](const pair<uuid, shared_ptr<room_t>> &room) { return room.second; });
  return move(vector(view.begin(), view.end()));
}

size_t room_list_t::get_limit() const {
  shared_lock lock(rooms_mutex);
  return limit;
}

void room_list_t::set_limit(const size_t new_limit) {
  lock_guard lock(rooms_mutex);
  limit = new_limit;
}

void room_list_t::clean(const chrono::milliseconds user_timeout) {
  for (const shared_ptr<room_t> &room: get_all()) {
    if (!room->is_available()) remove(room->id);
    room->kick_expired(user_timeout);
    room->clean_sync_records();
  }
}
