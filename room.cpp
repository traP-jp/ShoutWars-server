#include "room.hpp"

#include "errors.hpp"
#include <format>
#include <ranges>
#include <utility>

using namespace std;
using namespace boost::uuids;

using json = nlohmann::json;

uuid room_t::gen_id() {
  thread_local time_generator_v7 gen;
  return gen();
}

// room user

room_t::user_t::user_t(const string &name)
  : id(gen_id()), last_sync_id(nil_uuid()), last_time(chrono::steady_clock::now()) {
  set_name(name);
}

void to_json(json &j, const room_t::user_t &user) {
  j = { { "id", to_string(user.id) }, { "name", user.name } };
}

string room_t::user_t::get_name() const {
  return name;
}

void room_t::user_t::set_name(const string &new_name) {
  if (new_name.empty() || new_name.size() > name_max_length) {
    throw bad_request_error(
      format("Invalid user name length: {}. Must be between 1 and {}.", new_name.size(), name_max_length)
    );
  }
  name = new_name;
}

uuid room_t::user_t::get_last_sync_id() const {
  return last_sync_id;
}

chrono::steady_clock::time_point room_t::user_t::get_last_time() const {
  return last_time;
}

void room_t::user_t::update_last(const uuid new_sync_id) {
  last_sync_id = new_sync_id;
  last_time = chrono::steady_clock::now();
}

// room sync event

room_t::sync_record_t::event_t::event_t(const uuid id, const uuid from, string type, json data)
  : id(id), from(from), type(move(type)), data(move(data)) {}

void to_json(json &j, const room_t::sync_record_t::event_t &event) {
  j = {
    { "id", to_string(event.id) },
    { "from", to_string(event.from) },
    { "type", event.type },
    { "event", event.data }
  };
}

// room sync

room_t::sync_record_t::sync_record_t() : id(gen_id()) {}

void room_t::sync_record_t::add_events(
  const uuid from, const vector<shared_ptr<event_t>> &new_reports, const vector<shared_ptr<event_t>> &new_actions
) {
  lock_guard lock(record_mutex);
  if (users_phase[from] > phase_t::CREATED) throw bad_request_error("Record already synced.");
  for (const shared_ptr<event_t> &report: new_reports) {
    if (report->from != from) throw bad_request_error("Invalid report from.");
    reports[report->id] = report;
  }
  for (const shared_ptr<event_t> &action: new_actions) {
    if (action->from != from) throw bad_request_error("Invalid action from.");
    actions[action->id] = action;
  }
  users_phase[from] = phase_t::WAITING;
}

vector<shared_ptr<room_t::sync_record_t::event_t>> room_t::sync_record_t::get_reports() const {
  shared_lock lock(record_mutex);
  const auto view = reports | views::transform(
                      [](const pair<uuid, shared_ptr<event_t>> &report) { return report.second; }
                    );
  return move(vector(view.begin(), view.end()));
}

vector<shared_ptr<room_t::sync_record_t::event_t>> room_t::sync_record_t::get_actions() const {
  shared_lock lock(record_mutex);
  const auto view = actions | views::transform(
                      [](const pair<uuid, shared_ptr<event_t>> &action) { return action.second; }
                    );
  return move(vector(view.begin(), view.end()));
}

room_t::sync_record_t::phase_t room_t::sync_record_t::get_phase(const uuid user_id) {
  lock_guard lock(record_mutex);
  return users_phase[user_id];
}

bool room_t::sync_record_t::advance_phase(const uuid user_id, const phase_t new_phase) {
  lock_guard lock(record_mutex);
  if (new_phase <= users_phase[user_id]) return false;
  users_phase[user_id] = new_phase;
  return true;
}

room_t::sync_record_t::phase_t room_t::sync_record_t::get_max_phase() const {
  shared_lock lock(record_mutex);
  return ranges::max(users_phase | views::values);
}

// room

room_t::room_t(
  string version, const user_t &owner, size_t size, const chrono::minutes lobby_lifetime,
  const chrono::minutes game_lifetime, logger log_error, logger log_info
)
  : log_error(move(log_error)), log_info(move(log_info)), lobby_lifetime(lobby_lifetime), game_lifetime(game_lifetime),
    id(gen_id()), version(move(version)), size(size), expire_time(chrono::steady_clock::now() + lobby_lifetime),
    users{ { owner.id, owner } }, in_lobby(true) {
  if (this->version.empty() || this->version.size() > version_max_length) {
    throw bad_request_error(
      format("Invalid room version length: {}. Must be between 1 and {}.", this->version.size(), version_max_length)
    );
  }
  if (size < 2 || size > size_max) {
    throw bad_request_error(format("Invalid room size: {}. Must be between 2 and {}.", size, size_max));
  }
  const auto record = make_shared<sync_record_t>();
  sync_records.emplace(record->id, record);
  users.begin()->second.update_last(nil_uuid());
}

void room_t::join(string version, const user_t &user) {
  if (version != this->version) {
    throw bad_request_error(format("Invalid room version: {}. This roon version is {}.", version, this->version));
  }
  lock_guard lock(room_mutex);
  if (!in_lobby) throw forbidden_error("Game already started.");
  if (users.size() >= size) throw forbidden_error(format("Room is full. Max user count is {}.", size));
  if (users.contains(user.id)) throw forbidden_error("User already in the room.");
  user_t &new_user = users.emplace(user.id, user).first->second;
  new_user.update_last(sync_records.size() > 1 ? next(sync_records.rbegin())->first : nil_uuid());
}

room_t::user_t room_t::get_user(const uuid id) const {
  shared_lock lock(room_mutex);
  try {
    return users.at(id);
  } catch (const out_of_range &) {
    throw not_found_error("User not found.");
  }
}

bool room_t::has_user(const uuid id) const {
  shared_lock lock(room_mutex);
  return users.contains(id);
}

bool room_t::kick(const uuid id) {
  lock_guard lock(room_mutex);
  return users.erase(id) > 0;
}

size_t room_t::kick_expired(const chrono::milliseconds timeout) {
  lock_guard lock(room_mutex);
  const auto now = chrono::steady_clock::now();
  return erase_if(users, [&](const pair<uuid, user_t> &user) { return now - user.second.get_last_time() > timeout; });
}

size_t room_t::count_users() const {
  shared_lock lock(room_mutex);
  return users.size();
}

vector<uuid> room_t::get_user_ids() const {
  shared_lock lock(room_mutex);
  const auto view = users | views::keys;
  return move(vector(view.begin(), view.end()));
}

vector<room_t::user_t> room_t::get_users() const {
  shared_lock lock(room_mutex);
  const auto view = users | views::values;
  return move(vector(view.begin(), view.end()));
}

room_t::user_t room_t::get_owner() const {
  shared_lock lock(room_mutex);
  if (users.empty()) throw not_found_error("Room is empty.");
  return users.begin()->second;
}

bool room_t::is_in_lobby() const {
  shared_lock lock(room_mutex);
  return in_lobby;
}

void room_t::start_game() {
  shared_lock lock(room_mutex);
  if (!in_lobby) throw forbidden_error("Game already started.");
  if (users.size() < 2) throw forbidden_error("Not enough players to start the game.");
  in_lobby = false;
  expire_time = chrono::steady_clock::now() + game_lifetime;
  const auto users_view = users | views::values;
  log_info(
    format("Game started: {} (users={})", to_string(id), json(vector(users_view.begin(), users_view.end())).dump())
  );
}

bool room_t::is_available() const {
  shared_lock lock(room_mutex);
  if (chrono::steady_clock::now() > expire_time) return false;
  if (in_lobby) return !users.empty();
  return users.size() > 1;
}

json room_t::get_info() const {
  shared_lock lock(room_mutex);
  return info;
}

void room_t::update_info(const json &new_info) {
  lock_guard lock(room_mutex);
  info = new_info;
}

vector<shared_ptr<room_t::sync_record_t>> room_t::sync(
  const uuid user_id, const vector<shared_ptr<sync_record_t::event_t>> &reports,
  const vector<shared_ptr<sync_record_t::event_t>> &actions, const chrono::milliseconds wait_timeout,
  const chrono::milliseconds sync_timeout
) {
  unique_lock lock(room_mutex);
  if (!users.contains(user_id)) throw forbidden_error("User not in the room.");
  user_t &user = users.at(user_id);
  const shared_ptr<sync_record_t> record = sync_records.rbegin()->second;
  if (record->get_phase(user.id) > sync_record_t::phase_t::CREATED) throw forbidden_error("User already synced.");
  if (record->get_max_phase() >= sync_record_t::phase_t::SYNCED) throw forbidden_error("Room already synced.");

  record->add_events(user.id, reports, actions);

  // wait for users who didn't skip last sync
  if (record->get_max_phase() <= sync_record_t::phase_t::WAITING && sync_records.size() > 1) {
    if (next(sync_records.rbegin())->second->get_phase(user.id) < sync_record_t::phase_t::SYNCED) {
      sync_cv.wait_for(lock, wait_timeout, [&] { return record->get_max_phase() > sync_record_t::phase_t::WAITING; });
    }
  }
  record->advance_phase(user.id, sync_record_t::phase_t::SYNCING);
  sync_cv.notify_all();

  // wait for all users to sync
  if (ranges::any_of(
    users,
    [&](const pair<uuid, user_t> &u) { return record->get_phase(u.first) <= sync_record_t::phase_t::CREATED; }
  )) {
    sync_cv.wait_for(lock, sync_timeout, [&] { return record->get_max_phase() > sync_record_t::phase_t::SYNCING; });
  }
  record->advance_phase(user.id, sync_record_t::phase_t::SYNCED);
  sync_cv.notify_all();

  vector<shared_ptr<sync_record_t>> records;
  for (const shared_ptr<sync_record_t> &r: ranges::subrange(
                                             sync_records.upper_bound(user.get_last_sync_id()),
                                             sync_records.end()
                                           ) | views::values) {
    records.emplace_back(r);
    r->advance_phase(user.id, sync_record_t::phase_t::SYNCED);
  }
  // if all users synced, create new record
  if (ranges::all_of(
    users,
    [&](const pair<uuid, user_t> &u) {
      if (record->get_phase(u.first) <= sync_record_t::phase_t::CREATED) return true;
      return record->get_phase(u.first) >= sync_record_t::phase_t::SYNCED;
    }
  )) {
    const auto next_record = make_shared<sync_record_t>();
    sync_records.emplace(next_record->id, next_record);
  }
  user.update_last(record->id);
  return move(records);
}

size_t room_t::clean_sync_records() {
  lock_guard lock(room_mutex);
  return erase_if(
    sync_records,
    [&](const pair<uuid, shared_ptr<sync_record_t>> &record) {
      return ranges::all_of(
        users,
        [&](const pair<uuid, user_t> &user) {
          return record.second->get_phase(user.first) >= sync_record_t::phase_t::SYNCED;
        }
      );
    }
  );
}

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
