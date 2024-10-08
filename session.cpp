#include "session.hpp"

#include "errors.hpp"
#include <format>
#include <utility>

using namespace std;
using namespace boost::uuids;

uuid session_t::gen_id() {
  thread_local time_generator_v7 gen;
  return gen();
}

// session

session_t::session_t(const uuid room_id, const uuid user_id) : id(gen_id()), room_id(room_id), user_id(user_id) {}

// session list

session_list_t::session_list_t(logger log_error, logger log_info)
  : log_error(move(log_error)), log_info(move(log_info)) {}

session_t session_list_t::create(const uuid &room_id, const uuid &user_id) {
  lock_guard lock(sessions_mutex);
  const session_t session(room_id, user_id);
  sessions.emplace(session.id, session);
  log_info(
    format(
      "Session created: {} (room_id={}, user_id={})",
      to_string(session.id),
      to_string(room_id),
      to_string(user_id)
    )
  );
  return session;
}

session_t session_list_t::get(const uuid id) const {
  shared_lock lock(sessions_mutex);
  try {
    return sessions.at(id);
  } catch (const out_of_range &) {
    throw unauthorized_error("Session not found.");
  }
}

bool session_list_t::exists(const uuid id) const {
  shared_lock lock(sessions_mutex);
  return sessions.contains(id);
}

bool session_list_t::remove(const uuid id) {
  lock_guard lock(sessions_mutex);
  if (sessions.erase(id) > 0) {
    log_info(format("Session removed: {}", to_string(id)));
    return true;
  }
  return false;
}

size_t session_list_t::clean(const function<bool(const session_t &)> &is_expired) {
  lock_guard lock(sessions_mutex);
  return erase_if(sessions, [&](const pair<uuid, session_t> &session) {
    if (is_expired(session.second)) {
      log_info(format("Session expired: {}", to_string(session.first)));
      return true;
    }
    return false;
  });
}
