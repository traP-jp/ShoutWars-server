#pragma once

#include <boost/uuid.hpp>
#include <shared_mutex>
#include <map>
#include <string>
#include <functional>

class session_t {
public:
  const boost::uuids::uuid id;
  const boost::uuids::uuid room_id;
  const boost::uuids::uuid user_id;

  [[nodiscard]] explicit session_t(boost::uuids::uuid room_id, boost::uuids::uuid user_id);

protected:
  static boost::uuids::uuid gen_id();
};

class session_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  const logger log_error, log_info;

  [[nodiscard]] explicit session_list_t(
    logger log_error = [](const std::string &) {}, logger log_info = [](const std::string &) {}
  );

  [[nodiscard]] session_t create(const boost::uuids::uuid &room_id, const boost::uuids::uuid &user_id);

  [[nodiscard]] session_t get(boost::uuids::uuid id) const;

  [[nodiscard]] bool exists(boost::uuids::uuid id) const;

  bool remove(boost::uuids::uuid id);

  size_t clean(const std::function<bool(const session_t &)> &is_expired);

protected:
  mutable std::shared_mutex sessions_mutex;
  std::map<boost::uuids::uuid, session_t> sessions;
};
