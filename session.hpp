#pragma once

#include <boost/uuid/time_generator_v7.hpp>
#include <boost/uuid/uuid.hpp>
#include <mutex>
#include <map>
#include <functional>

class session_t {
public:
  const boost::uuids::uuid id;
  const boost::uuids::uuid room_id;
  const boost::uuids::uuid user_id;

  explicit session_t(boost::uuids::uuid room_id, boost::uuids::uuid user_id);

protected:
  static boost::uuids::time_generator_v7 gen_id;
};

class session_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  logger log_info, log_error;

  explicit session_list_t(logger log_info, logger log_error);

  session_t create(const boost::uuids::uuid &room_id, const boost::uuids::uuid &user_id);

  session_t get(boost::uuids::uuid id) const;

  bool exists(boost::uuids::uuid id) const;

  bool remove(boost::uuids::uuid id);

protected:
  std::map<boost::uuids::uuid, session_t> sessions;
  mutable std::mutex sessions_mutex;
};
