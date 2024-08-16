#include "session.hpp"

#include <utility>

using namespace std;
using namespace boost::uuids;

time_generator_v7 session_t::gen_id{};

// session

session_t::session_t(const uuid room_id, const uuid user_id) : id(gen_id()), room_id(room_id), user_id(user_id) {}

// session list

session_list_t::session_list_t(logger log_info, logger log_error)
  : log_info(move(log_info)), log_error(move(log_error)) {}

session_t session_list_t::create(const uuid &room_id, const uuid &user_id) {
  lock_guard lock(sessions_mutex);
  const session_t session(room_id, user_id);
  sessions.emplace(session.id, session);
  return session;
}

session_t session_list_t::get(const uuid id) const {
  lock_guard lock(sessions_mutex);
  return sessions.at(id);
}

bool session_list_t::exists(const uuid id) const {
  lock_guard lock(sessions_mutex);
  return sessions.contains(id);
}

session_t session_list_t::remove(const uuid id) {
  lock_guard lock(sessions_mutex);
  const auto session = sessions.at(id);
  sessions.erase(id);
  return session;
}
