#include "room.hpp"

#include "errors.hpp"
#include <utility>

using namespace std;
using namespace boost::uuids;

time_generator_v7 room_t::gen_id{};

// room user

room_t::user_t::user_t(string name)
  : id(gen_id()), name(move(name)) {
  if (this->name.empty() || this->name.size() > 32) {
    throw bad_request_error(format("Invalid user name length: {}. Must be between 1 and 32.", this->name.size()));
  }
}

room_t::user_t::user_t(const user_t &other) : id(other.id), name(other.name) {}

string room_t::user_t::get_name() const {
  lock_guard lock(name_mutex);
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

room_t::room_t(string version, const user_t &owner, const size_t size)
  : id(gen_id()), version(move(version)), size(size), users{ owner } {
  if (size < 2 || size > 4) throw bad_request_error(format("Invalid room size: {}. Must be between 2 and 4.", size));
}

// room list

room_list_t::room_list_t(logger log_info, logger log_error, const size_t limit)
  : log_info(move(log_info)), log_error(move(log_error)), limit(limit) {}

shared_ptr<room_t> room_list_t::create(const string &version, const room_t::user_t &owner, const size_t size) {
  lock_guard lock(rooms_mutex);
  if (rooms.size() >= limit) throw forbidden_error(format("Room limit reached. Max room count is {}.", limit));
  auto room = make_shared<room_t>(version, owner, size);
  rooms[room->id] = room;
  return room;
}

shared_ptr<room_t> room_list_t::get(const uuid id) const {
  lock_guard lock(rooms_mutex);
  try {
    return rooms.at(id);
  } catch (const out_of_range &) {
    throw not_found_error("Room not found.");
  }
}

bool room_list_t::exists(const uuid id) const {
  lock_guard lock(rooms_mutex);
  return rooms.contains(id);
}

bool room_list_t::remove(const uuid id) {
  lock_guard lock(rooms_mutex);
  return rooms.erase(id) > 0;
}

size_t room_list_t::count() const {
  lock_guard lock(rooms_mutex);
  return rooms.size();
}

vector<shared_ptr<room_t>> room_list_t::get_all() const {
  lock_guard lock(rooms_mutex);
  vector<shared_ptr<room_t>> result;
  result.reserve(rooms.size());
  for (const auto &room: rooms | views::values) result.push_back(room);
  return result;
}

bool room_list_t::start_cleaner(const std::chrono::milliseconds interval, const std::chrono::milliseconds timeout) {
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
  is_cleaner_running = false;
  if (cleaner.joinable()) cleaner.join();
}
