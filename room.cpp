#include "room.hpp"

#include "errors.hpp"
#include <boost/uuid/uuid_io.hpp>
#include <format>
#include <ranges>
#include <utility>

using namespace std;
using namespace boost::uuids;

using json = nlohmann::json;

time_generator_v7 room_t::gen_id{};

// room user

room_t::user_t::user_t(string name)
  : id(gen_id()), name(move(name)) {
  if (this->name.empty() || this->name.size() > 32) {
    throw bad_request_error(format("Invalid user name length: {}. Must be between 1 and 32.", this->name.size()));
  }
}

room_t::user_t::user_t(const user_t &other) : id(gen_id()), name(other.name), last_time(other.last_time.load()) {}

void to_json(json &j, const room_t::user_t &user) {
  j = { { "id", to_string(user.id) }, { "name", user.name } };
}

string room_t::user_t::get_name() const {
  shared_lock lock(name_mutex);
  return name;
}

void room_t::user_t::set_name(string new_name) {
  lock_guard lock(name_mutex);
  if (new_name.empty() || new_name.size() > 32) {
    throw bad_request_error(format("Invalid user name length: {}. Must be between 1 and 32.", new_name.size()));
  }
  name = move(new_name);
}

// room

room_t::room_t(string version, const user_t &owner, size_t size, logger log_error, logger log_info)
  : log_error(move(log_error)), log_info(move(log_info)), id(gen_id()), version(move(version)), size(size),
    users{ owner } {
  if (this->version.empty() || this->version.size() > 32) {
    throw bad_request_error(format("Invalid room version length: {}. Must be between 1 and 32.", this->version.size()));
  }
  if (size < 2 || size > 4) throw bad_request_error(format("Invalid room size: {}. Must be between 2 and 4.", size));
}

void room_t::join(std::string version, const user_t &user) {
  if (version != this->version) {
    throw bad_request_error(format("Invalid room version: {}. This roon version is {}.", version, this->version));
  }
  if (!in_lobby) throw forbidden_error("Game already started.");
  lock_guard lock(users_mutex);
  if (users.size() >= size) throw forbidden_error(format("Room is full. Max user count is {}.", size));
  if (ranges::any_of(users, [=](const user_t &u) { return u.id == user.id; })) {
    throw forbidden_error("User already in the room.");
  }
  users.push_back(user);
}

room_t::user_t room_t::get_user(const uuid id) const {
  shared_lock lock(users_mutex);
  for (const auto &user: users) if (user.id == id) return user;
  throw not_found_error("User not found.");
}

bool room_t::has_user(const uuid id) const {
  shared_lock lock(users_mutex);
  return ranges::any_of(users, [=](const user_t &user) { return user.id == id; });
}

bool room_t::kick(const uuid id) {
  lock_guard lock(users_mutex);
  for (auto it = users.begin(); it != users.end(); ++it) {
    if (it->id == id) {
      users.erase(it);
      return true;
    }
  }
  return false;
}

size_t room_t::kick_expired(const chrono::milliseconds timeout) {
  lock_guard lock(users_mutex);
  const auto now = chrono::steady_clock::now();
  size_t count = 0;
  for (auto it = users.begin(); it != users.end();) {
    if (now - it->last_time.load() > timeout) {
      it = users.erase(it);
      ++count;
    } else {
      ++it;
    }
  }
  return count;
}

list<room_t::user_t> room_t::get_users() const {
  shared_lock lock(users_mutex);
  return users;
}

bool room_t::is_in_lobby() const {
  return in_lobby;
}

void room_t::start_game() {
  if (!in_lobby) throw forbidden_error("Game already started.");
  shared_lock lock(users_mutex);
  if (users.size() < 2) throw forbidden_error("Not enough players to start the game.");
  in_lobby = false;
  log_info(format("Game started: {} (users={})", to_string(id), json(users).dump()));
}

const json &room_t::get_info() const {
  lock_guard lock(info_mutex);
  return info;
}

void room_t::update_info(const json &new_info) {
  lock_guard lock(info_mutex);
  info = new_info;
}

// room list

room_list_t::room_list_t(const size_t limit, logger log_error, logger log_info)
  : log_error(move(log_error)), log_info(move(log_info)), limit(limit) {}

shared_ptr<room_t> room_list_t::create(const string &version, const room_t::user_t &owner, const size_t size) {
  lock_guard lock(rooms_mutex);
  if (rooms.size() >= limit) throw forbidden_error(format("Room limit reached. Max room count is {}.", limit.load()));
  auto room = make_shared<room_t>(version, owner, size);
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

list<shared_ptr<room_t>> room_list_t::get_all() const {
  shared_lock lock(rooms_mutex);
  list<shared_ptr<room_t>> result;
  for (const auto &room: rooms | views::values) result.push_back(room);
  return result;
}

bool room_list_t::start_cleaner(const std::chrono::milliseconds interval, const std::chrono::milliseconds timeout) {
  lock_guard lock(cleaner_mutex);
  if (cleaner.joinable()) return false;
  is_cleaner_running = true;
  cleaner = thread(
    [=, this] {
      while (is_cleaner_running) {
        const auto now = chrono::steady_clock::now();
        for (const auto &room: get_all()) {
          // TODO
        }
        this_thread::sleep_for(interval);
      }
    }
  );
  return true;
}

void room_list_t::stop_cleaner() {
  lock_guard lock(cleaner_mutex);
  is_cleaner_running = false;
  if (cleaner.joinable()) cleaner.join();
}
